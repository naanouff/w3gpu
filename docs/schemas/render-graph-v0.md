# Schéma render graph w3drs — **v0**

**Statut** : spécification **figée pour parse/validation** ; l’exécuteur GPU (natif + **WASM** avec sources WGSL injectées) couvre les passes listées ci‑dessous. **WASM** : `w3drsValidateRenderGraphV0` (parse + `validate_exec_v0`) et **`W3drsEngine` → `w3drsPhaseBGraphRunChecksum`** (encode + readback, même logique que `run_graph_v0_checksum` natif).

## En-tête

| Champ | Type | Contrainte |
|-------|------|-------------|
| `schema` | string | exactement `w3drs.render_graph` |
| `version` | u32 | `1` |
| `resources` | array | ids **uniques** |
| `passes` | array | non vide ; ids de passe **uniques** ; ordre = ordre de soumission cible |

## Ressources (`resources[]`)

Discriminant : `kind`.

### `texture_2d`

| Champ | Type | Description |
|-------|------|-------------|
| `id` | string | Identifiant stable dans le graphe. |
| `format` | string | Formats **couleur** reconnus : `Rgba16Float`, `Rgba8Unorm`. Formats **profondeur** (validation + GPU natif si utilisés comme `depth_target`) : `Depth24Plus`, `Depth32Float`, `Depth24PlusStencil8`, `Depth32FloatStencil8`. La cible **`run_graph_v0_checksum`** (`readback_id`) doit rester une texture **`Rgba16Float`** (readback fixe à 8 octets / texel côté outil). |
| `width` / `height` | u32 | Taille en pixels (**mip 0**). |
| `usage` | string[] | *Optionnel* — usages logiques ; la validation **B.2** exige que les passes déclarées soient **compatibles** avec ces flags (ex. `raster_mesh.color_targets` → `render_attachment` ; `compute.storage_writes` → `storage` ; `compute.texture_reads` → `texture_binding` ; **`blit`** : `source` → `copy_src`, `destination` → `copy_dst`). |
| `mip_level_count` | u32 | *Optionnel*, défaut `1`, plage **1..=32** : nombre de mips alloués sur le GPU. Requis pour les blits avec `region.src_mip_level` / `dst_mip_level` > 0. |

### `buffer`

| Champ | Type | Description |
|-------|------|-------------|
| `id` | string | Identifiant stable. |
| `size` | u64 | Taille octets. |
| `usage` | string[] | *Optionnel*. Flags reconnus : `storage`, `copy_dst`, `copy_src`, `map_read`, **`indirect`**, **`uniform`** (B.7 : `raster_depth_mesh` — buffer `light_uniforms`). |

## Passes (`passes[]`)

Discriminant : `kind`.

Toutes les variantes (v0) acceptent en option : **`ecs_before`**, **`ecs_after`** (string) — sémantique B.6 : l’hôte moteur implémente `RenderGraphV0Host::ecs_node` (natif) ; exécution entrecoupée de l’encode **avant** / **après** la passe (labels non vides ; champ absent = pas d’appel).

### `compute`

| Champ | Type | Contrainte |
|-------|------|------------|
| `id` | string | Unique. |
| `shader` | string | Chemin **relatif** au fichier graphe (ex. `shaders/foo.wgsl`). |
| `entry_point` | string | Nom fonction compute. |
| `dispatch` | object | `x`, `y`, `z` > 0 (requis pour le parse ; **ignoré** côté GPU si `indirect_dispatch` est présent). |
| `texture_reads` | string[] | *Optionnel* — ids `texture_2d` **lus** en entrée *sampled* (`textureLoad` / `texture_2d<f32>` WGSL) ; `texture_binding` + format couleur **v0** (`Rgba16Float`, `Rgba8Unorm` seulement) ; pas de doublon ; id **interdit** s’il apparaît aussi dans `storage_writes`. Bindings **group 0** : `binding = len(storage_buffers) + len(storage_buffers_read) + len(storage_writes) + index`. |
| `storage_writes` | string[] | *Optionnel* — ids `texture_2d` **écrits** en storage (usage `storage`) ; bindings **group 0** : indices `len(storage_buffers) + len(storage_buffers_read)` … `+ len(storage_writes) - 1` (ordre de la liste, pas de doublon). |
| `storage_buffers_read` | string[] | *Optionnel* — buffers `storage` **en lecture seule** (`var<storage, read>`), après les `storage_buffers` rw ; mêmes règles d’usage `storage` ; pas de doublon ; un id ne peut figurer **ni** dans `storage_buffers` **ni** deux fois ici. |
| `storage_buffers` | string[] | *Optionnel* — ids `buffer` en **read/write storage** (`var<storage, read_write>`), **group 0** (`binding` = index dans cette liste). Chaque buffer doit déclarer `storage` ; l’exécuteur natif crée layout + bind group + `set_bind_group(0, …)` avant le dispatch. |
| `storage_buffers_group1` | string[] | *Optionnel* — ids `buffer` en **read/write storage** dans **`@group(1)`** (`binding` = index dans cette liste). |
| `storage_buffers_read_group1` | string[] | *Optionnel* — buffers `storage` en **lecture seule** dans **`@group(1)`**, après `storage_buffers_group1` (`binding` = `len(storage_buffers_group1) + index`). Pas de doublon ; un id ne peut figurer **ni** dans `storage_buffers_group1` **ni** dans le groupe 0. |
| `indirect_dispatch` | object \| absent | Si présent : `buffer` (id), `offset` (u64, multiple de 4, `offset+12 ≤ size` du buffer) ; le buffer doit déclarer **`indirect`** ; l’exécuteur natif appelle `dispatch_workgroups_indirect` (12 octets : 3×`u32` little-endian `x`,`y`,`z`) au lieu de `dispatch`. |
| *(groupe 1)* | — | Si **`storage_buffers_group1`** ou **`storage_buffers_read_group1`** est non vide, le groupe 0 doit avoir **au moins** une ressource (buffers rw/ro, `storage_writes`, ou `texture_reads`). L’exécuteur natif appelle `set_bind_group(1, …)` après le groupe 0. |

### `raster_mesh`

| Champ | Type | Description |
|-------|------|-------------|
| `id` | string | Unique. |
| `shader` | string | Module WGSL (vertex + fragment). |
| `vertex_entry` / `fragment_entry` | string | Points d’entrée. |
| `color_targets` | string[] | *Optionnel* — ids de textures attachées en couleur (chaque id **au plus une fois** par passe ; usages `render_attachment` requis). |
| `depth_target` | string \| null | *Optionnel* — id `texture_2d` ; le **format** doit être depth/stencil ; `render_attachment` requis sur la ressource. L’exécuteur v0 **attache** la depth au render pass (`clear` 1.0) ; la taille doit **matcher** les `color_targets` (même extent mip 0). |

### `fullscreen`

Même champs et mêmes règles de validation / GPU natif que **`raster_mesh`** dans v0 (triangle sans VB, un color attachment + depth optionnelle). Le discriminant sert à **marquer** l’intention « passe fullscreen déclarative » dans le graphe (post-process, etc.).

### `raster_depth_mesh` (B.7)

Passe **raster** sans couleur : ombre *depth-only*, même layout WGSL de référence que le moteur `shadow_depth` (group(0) uniform, group(1) `storage, read` matrices d’instances, vertex @location(0) position, mesh encodé par l’hôte).

| Champ | Type | Contrainte |
|-------|------|------------|
| `id` | string | Unique. |
| `shader` | string | Module WGSL (vertex seul, pas de `fragment` encodé). |
| `vertex_entry` | string | Ex. `vs_main`. |
| `depth_target` | string | id `texture_2d` en format profondeur ; `render_attachment`. |
| `light_uniforms_buffer` | string | id `buffer` (≥ 80 o, `uniform`) — ex. 80B `LightUniforms`. |
| `instance_buffer` | string | id `buffer` (`storage`) — au moins 64 o (une matrice). |

Dessin des mesh (indices / VB) : **pas** en JSON — [`RenderGraphV0Host::draw_raster_depth_mesh`](../../crates/w3drs-renderer/src/render_graph_exec.rs) ; `w3drs-wasm` peut utiliser l’hôte `Noop` (pas d’`encode` ombre côté web si non câblé). Registre : [`insert_buffer`](../../crates/w3drs-renderer/src/render_graph_exec.rs) / `insert_texture_2d` pour câbler les ressources moteur.

### `blit`

| Champ | Type | Contrainte |
|-------|------|-------------|
| `id` | string | Unique. |
| `source` | string | id `texture_2d` ; même **format** que `destination` ; usage **`copy_src`** requis. |
| `destination` | string | id `texture_2d` ; usage **`copy_dst`** requis. |
| `region` | object \| absent | *Optionnel.* Sous-copie : `src_mip_level` / `dst_mip_level` (défaut `0`), origines texels `src_origin_*` / `dst_origin_*` (défaut `0`), `width` / `height` optionnels (implicite = rectangle maximal valide dans les deux mips). Les mips doivent être `< mip_level_count` de chaque texture ; l’extent copié doit être identique source/destination. **Absent** : copie **mip 0** entière ; exige même **extent** logique (mip 0) entre les deux textures. |

## Référence code

- Crate **`w3drs-render-graph`** : `parse_render_graph_json`, erreurs `RenderGraphError`.
- **Validation exécution v0 (sans GPU)** : `w3drs_render_graph::validate_exec_v0` — partagée **natif / WASM** ; erreurs `RenderGraphValidateError`. Inclut la couche **B.2** (usages sémantiques par passe, doublons `color_targets`, `depth_target` cohérent, blit région/mips, indirect dispatch) + `pass_ids_in_order_v0` (ordre linéaire des passes pour un futur planificateur de barrières). Côté renderer natif : `validate_render_graph_exec_v0` (alias + conversion vers `RenderGraphExecError`). Côté **WASM** : `w3drsValidateRenderGraphV0` dans `www/pkg` (`npm run build:wasm`).
- **Registre GPU v0 (Phase B.1)** : `w3drs_renderer::RenderGraphGpuRegistry` — allocation nommée textures / buffers depuis le JSON (`mip_level_count` sur textures) ; `resize_texture_2d` (conserve le nombre de mips) ; exécution sur registre existant via `run_graph_v0_checksum_with_registry` / **`run_graph_v0_checksum_with_registry_pre_writes`** (écritures buffer optionnelles avant encode, ex. tests indirect).
- **Exécuteur natif (v0)** : `encode_render_graph_passes_v0` / `encode_render_graph_passes_v0_with_wgsl` ; variants avec hôte B.6/B.7 : `encode_render_graph_passes_v0_with_wgsl_host`, `run_graph_v0_checksum_with_registry_wgsl_host` — `NoopRenderGraphV0Host` = comportement historique. `run_graph_v0_checksum` — `wgpu::Device` / `Queue`, répertoire des shaders (ex. `fixtures/phases/phase-b/`).
- Fixture : [`fixtures/phases/phase-b/render_graph.json`](../../fixtures/phases/phase-b/render_graph.json) ; B.6/B.7 (smoke) : [`b67_raster_depth_test.json`](../../fixtures/phases/phase-b/b67_raster_depth_test.json) + [`shaders/shadow_depth.wgsl`](../../fixtures/phases/phase-b/shaders/shadow_depth.wgsl).
- Exemple natif : `cargo run -p phase-b-graph --release` ([`examples/phase-b-graph`](../../examples/phase-b-graph)) — charge le JSON, valide, exécute `run_graph_v0_checksum`, affiche le hachage sur **stderr**.
- Viewer PBR : `cargo run -p khronos-pbr-sample -- --render-graph fixtures/phases/phase-b/render_graph.json` ; options **`--render-graph-readback`**, **`--render-graph-slot`** `pre` \| `after_cull` \| `post_pbr` (emplacement d’encodage dans la frame). **WASM** : `www` — `w3drsPhaseBGraphRunChecksum` après `fetch` du JSON + des `shaders/*.wgsl` (voir `www/src/main.ts`).

## Feuille de route — exécuteur complet (parité moteur, objectif w3dts)

Le v0 prouve **parse + validation + soumission wgpu minimale** (checksum) sur **natif**. La suite, pour s’aligner sur un *RenderGraph* w3dts **data-driven** branché au viewer principal, est planifiée comme suit (détail aussi : [Phase B — plan](../tickets/phase-B-graphe-rendu-compute.md#plan-dexécution--exécuteur-complet--wasm-cible-w3dts)) :

1. **Registre de ressources** — lifecycle (création, **resize** v0 sur `texture_2d`, **mips**, alias *à venir*). **Barrières (B.2)** : validation statique usages / passes + ordre linéaire (`pass_ids_in_order_v0`) **fait** ; insertion **wgpu** explicite / DAG *à venir* ; erreurs d’incompatibilité détectées **avant** submit dans la mesure du schéma v0.
2. **Généralisation des passes** — raster (fullscreen, mesh), compute (dispatch fixe **et indirect v0**), bind groups / layouts dérivés du schéma ; **blit** régions et mips **v0 natif**.
3. **Intégration moteur** — B.4 : hook CLI + emplacement d’encodage dans `khronos-pbr-sample` (**fait**) ; *suite* : remplacer des passes moteur par le même JSON (HDR / depth partagés).
4. **WASM** — B.5 : encode + checksum sur `Device/Queue` WebGPU (**fait** pour la fixture) ; *suite* : parité CI natif / web, perf.
5. **ECS (B.6)** — **fait (v0)** : champs `ecs_before` / `ecs_after` + hôte `ecs_node` ; *suite* : éditeur / libellés imposés par l’UI.
6. **Shadow (B.7)** — **fait (v0)** : `raster_depth_mesh` + hôte `draw_raster_depth_mesh` (layout aligné sur [`ShadowPass`](../../crates/w3drs-renderer/src/shadow_pass.rs)) ; *suite* : remplacer l’encode ombre manuel du viewer par ce hook + ressources injectées.

*Éléments historiquement « évolutions futures » désormais en v0 natif :* **blit** étendu (région, mips), **compute indirect** (dispatch depuis buffer), **`mip_level_count`**, **B.6** / **B.7** (hôtes).

## Évolutions futures (hors v0)

- Bindings / groupes explicites par passe.
- Liens ressource → slot ; barrières dérivées ou explicites.
- **Passes additionnelles** (ticket [Phase B — types de passes](../tickets/phase-B-graphe-rendu-compute.md#types-de-passes--v0-actuel-vs-backlog-phase-b)) : *clear* / *resolve* / *copy_to_buffer* / barrières nommées.
