# Schéma render graph w3drs — **v0**

**Statut** : spécification **figée pour parse/validation** ; l’exécuteur GPU est en cours de conception (Phase B).

## En-tête

| Champ | Type | Contrainte |
|-------|------|------------|
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
| `format` | string | Formats **couleur** reconnus : `Rgba16Float`, `Rgba8Unorm`. Formats **profondeur** (validation + GPU natif si utilisés comme `depth_target`) : `Depth24Plus`, `Depth32Float`, `Depth24PlusStencil8`, `Depth32FloatStencil8`. La cible **`run_graph_v0_checksum`** (`readback_id`) doit rester une texture **`Rgba16Float`** (readback fixe à 8 octets / texel côté outil).
| `width` / `height` | u32 | Taille en pixels. |
| `usage` | string[] | *Optionnel* — usages logiques ; la validation **B.2** exige que les passes déclarées soient **compatibles** avec ces flags (ex. `raster_mesh.color_targets` → la texture doit déclarer `render_attachment` ; `compute.storage_writes` → `storage` ; `compute.texture_reads` → `texture_binding`). |

### `buffer`

| Champ | Type | Description |
|-------|------|-------------|
| `id` | string | Identifiant stable. |
| `size` | u64 | Taille octets. |
| `usage` | string[] | *Optionnel*. |

## Passes (`passes[]`)

Discriminant : `kind`.

### `compute`

| Champ | Type | Contrainte |
|-------|------|------------|
| `id` | string | Unique. |
| `shader` | string | Chemin **relatif** au fichier graphe (ex. `shaders/foo.wgsl`). |
| `entry_point` | string | Nom fonction compute. |
| `dispatch` | object | `x`, `y`, `z` > 0. |
| `texture_reads` | string[] | *Optionnel* — ids `texture_2d` **lus** comme textures bindées dans cette passe compute (doivent déclarer `texture_binding`). |
| `storage_writes` | string[] | *Optionnel* — ids `texture_2d` **écrits** en storage dans cette passe (doivent déclarer `storage`). |

### `raster_mesh`

| Champ | Type | Description |
|-------|------|-------------|
| `id` | string | Unique. |
| `shader` | string | Module WGSL (vertex + fragment). |
| `vertex_entry` / `fragment_entry` | string | Points d’entrée. |
| `color_targets` | string[] | *Optionnel* — ids de textures attachées en couleur (chaque id **au plus une fois** par passe ; usages `render_attachment` requis). |
| `depth_target` | string \| null | *Optionnel* — id `texture_2d` ; le **format** doit être depth/stencil ; `render_attachment` requis sur la ressource. L’exécuteur v0 **attache** la depth au render pass (`clear` 1.0) ; la taille doit **matcher** les `color_targets` (même extent). |

## Référence code

- Crate **`w3drs-render-graph`** : `parse_render_graph_json`, erreurs `RenderGraphError`.
- **Validation exécution v0 (sans GPU)** : `w3drs_render_graph::validate_exec_v0` — partagée **natif / WASM** ; erreurs `RenderGraphValidateError`. Inclut la couche **B.2** (usages sémantiques par passe, doublons `color_targets`, `depth_target` cohérent) + `pass_ids_in_order_v0` (ordre linéaire des passes pour un futur planificateur de barrières). Côté renderer natif : `validate_render_graph_exec_v0` (alias + conversion vers `RenderGraphExecError`). Côté **WASM** : `w3drsValidateRenderGraphV0` dans `www/pkg` (`npm run build:wasm`).
- **Registre GPU v0 (Phase B.1)** : `w3drs_renderer::RenderGraphGpuRegistry` — allocation nommée textures / buffers depuis le JSON ; `resize_texture_2d` ; exécution sur registre existant via `run_graph_v0_checksum_with_registry` (utile quand la taille runtime diffère du document le temps d’aligner le schéma).
- **Exécuteur natif (v0)** : `run_graph_v0_checksum` — `wgpu::Device` / `Queue`, répertoire des shaders (ex. `fixtures/phases/phase-b/`).
- Fixture : [`fixtures/phases/phase-b/render_graph.json`](../../fixtures/phases/phase-b/render_graph.json).
- Exemple natif : `cargo run -p phase-b-graph --release` ([`examples/phase-b-graph`](../../examples/phase-b-graph)) — charge le JSON, valide, exécute `run_graph_v0_checksum`, affiche le hachage sur **stderr**.

## Feuille de route — exécuteur complet (parité moteur, objectif w3dts)

Le v0 prouve **parse + validation + soumission wgpu minimale** (checksum) sur **natif**. La suite, pour s’aligner sur un *RenderGraph* w3dts **data-driven** branché au viewer principal, est planifiée comme suit (détail aussi : [Phase B — plan](../tickets/phase-B-graphe-rendu-compute.md#plan-dexécution--exécuteur-complet--wasm-cible-w3dts)) :

1. **Registre de ressources** — lifecycle (création, **resize** v0 sur `texture_2d`, alias *à venir*). **Barrières (B.2)** : validation statique usages / passes + ordre linéaire (`pass_ids_in_order_v0`) **fait** ; insertion **wgpu** explicite / DAG *à venir* ; erreurs d’incompatibilité détectées **avant** submit dans la mesure du schéma v0.
2. **Généralisation des passes** — raster (fullscreen, mesh), compute (dispatch fixe → indirect quand le schéma l’admettra), bind groups / layouts décrits ou dérivés du schéma.
3. **Intégration moteur** — remplacer ou paramétrer les passes **codées** du [viewer PBR](../../crates/w3drs-renderer/) par le même graphe (même JSON fixture étendu ou graphe *projet*).
4. **WASM** — exécuter le graphe **GPU** dans le paquet `www/pkg` (pas seulement `w3drsValidateRenderGraphV0` : mêmes étapes d’*encode* que le natif, profils *threads* / perf documentés), puis tableau **natif vs web** pour la fixture phase-b.
5. **ECS** — attachement de systèmes (préparer buffers / uniforms) à des nœuds ou *labels* de passes (spec ultérieure).

*Éléments listés auparavant comme *évolutions futures* :* bindings explicites par passe, liens ressource → slot, passes fullscreen *vs* mesh, compute indirect.

## Évolutions futures (hors v0)

- Bindings / groupes explicites par passe.
- Liens ressource → slot ; barrières dérivées ou explicites.
- Passes fullscreen vs mesh ; compute indirect.
