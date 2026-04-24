# Journal d'implémentation

## Phase 0 — Scaffold

**Réalisé :**
- Workspace Cargo multi-crates (`w3drs-math`, `w3drs-ecs`, `w3drs-assets`, `w3drs-renderer`, `w3drs-wasm`)
- `.cargo/config.toml` avec `target-feature=+simd128` pour WASM
- `cargo xtask` runner avec commandes `www`, `client`, `check`, `setup-hooks`
- Pre-commit hook (`cargo check` native + wasm32)
- Projet Vite (`www/`) consommant le package WASM
- Git LFS configuré pour `*.glb`, `*.gltf`, `*.bin`, `*.hdr`, `*.exr`

**Décisions :**
- `wasm-pack --target web` (pas `bundler`) pour compatibilité Vite
- `npm.cmd` sur Windows (pas `npm`) dans xtask
- `wasm-pack --out-dir ../../www/pkg` (path relatif depuis le crate, pas le CWD)

---

## Phase 1 — Triangle WebGPU

**Réalisé :**
- `GpuContext` : init wgpu Instance/Adapter/Device/Queue/Surface
- Pipeline hardcodé WGSL, rendu d'un triangle coloré
- `W3drsEngine::tick(dt)` → render pass → submit
- `requestAnimationFrame` TypeScript
- Client natif `khronos-pbr-sample` avec winit

**Décisions :**
- Surface lifetime : `unsafe { std::mem::transmute(surface) }` pour `'static` sur native
- `Backends::BROWSER_WEBGPU` en wasm32, `Backends::all()` en native

---

## Phase 2 — ECS + PBR + glTF + Textures

**Réalisé :**

### ECS
- `World` avec `HashMap<TypeId, Box<dyn AnyStorage>>`
- Composants : `TransformComponent` (TRS + world matrix), `CameraComponent` (view/proj + frustum), `RenderableComponent` (mesh_id + mat_id), `CulledComponent` (tag)
- Systèmes : `transform_system`, `camera_system`, `frustum_culling_system`
- `Scheduler` avec liste de systèmes

### Renderer
- `AssetRegistry` : registre GPU avec IDs opaques
  - Fallback textures 1×1 créés à l'init (white, flat-normal, default-mr, black)
  - Material id=0 toujours disponible
  - `upload_texture_rgba8(data, w, h, srgb, device, queue) → u32`
- `RenderState` : pipeline PBR, bind group layouts (groups 0-3)
- Depth buffer `Depth32Float` recréé au resize
- `ObjectUniforms` avec dynamic offset (256-byte aligned, max 1024 objets)

### PBR (pbr.wgsl)
- Cook-Torrance BRDF : GGX (D) + Smith/Schlick-GGX (G) + Fresnel (F)
- Normal mapping TBN
- Tone mapping Reinhard

### Corrections bugs (importantes)
- **FrameUniforms size mismatch** : `vec3<f32>` en WGSL a alignement 16 bytes → padding implicite → taille 272 vs 256 attendu. Fix : utiliser des champs `f32` individuels pour le padding (`_pad2a`, `_pad2b`, `_pad2c`).
- **`vulkan` → `vulkan-portability`** : feature wgpu renommée en v24.
- **`gltf::import_slice`** : nécessite `features = ["import"]` dans Cargo.toml.
- **glTF texture type** : `pbr.base_color_texture()` retourne `Option<Info<'_>>` pas `Option<Texture<'_>>` → extraire `.texture().source().index()`.

### glTF Loader
- `gltf::import_slice` (buffers + images intégrés)
- Conversion vers `Vertex` 80-byte interleaved
- Fallback tangents/bitangents si absents dans le modèle
- `to_rgba8()` convertit tous les formats d'image (R8, R8G8B8, R16*, float32) vers RGBA8

---

## Phase 3 — IBL (Image-Based Lighting)

**Réalisé :**

### HDR Loader (`w3drs-assets/src/hdr_loader.rs`)
- `load_hdr_from_bytes(bytes) → HdrImage { pixels: Vec<[f32;3]>, width, height }`
- Utilise `image::load_from_memory_with_format` avec `ImageFormat::Hdr`

### Précomputation CPU (`w3drs-renderer/src/ibl.rs`)
- **Irradiance** 32×32×6 faces : intégration cosine-weighted (512 samples Hammersley)
- **Prefiltered env** 128×128×6×5mips : GGX importance sampling (256 samples / pixel)
- **BRDF LUT** 256×256 : split-sum IBL avec géométrie `k = roughness²/2` (256 samples)
- Upload GPU en `rgba16float` / `rg16float` (conversion f32→f16 inline)
- `IblContext::new_default()` : cubemap gris constant (ambient = 0.12) pour fallback
- `IblContext::from_hdr()` : précomputation complète (~53M samples, ~2s WASM)

### Shader (`pbr.wgsl` group 3)
- `textureSample(irradiance_map, ibl_sampler, n)` → diffuse IBL
- `textureSampleLevel(prefiltered_map, ibl_sampler, reflect(-v,n), roughness*4.0)` → specular
- `textureSample(brdf_lut, ibl_sampler, vec2(NdotV, roughness)).rg` → scale/bias F0
- `fresnel_schlick_roughness()` pour terme Fresnel indirect

### API WASM
- `engine.load_hdr(bytes: Uint8Array) → HdrLoadStats` (`.free()` côté JS) — appelé avant `load_gltf`
- `www/src/hdrLoadTimings.ts` + `main.ts` : mêmes mesures ; `window.w3drsHdrLoadTimings` (succès) ; **tests** `npm test` (Vitest) ; E2E `npm run test:e2e` (WebGPU, souvent `--headed`) ; chrono HDR WASM = crate **`instant`** avec la feature **`wasm-bindgen`** (sinon l’hôte devrait fournir `env::now` ; pas `std::time::Instant` en wasm32).
- **Natif** : `khronos-pbr-sample` log `HDR (natif) parse=… ibl=… env_bind=… total=…` au démarrage.

### Fix rotation casque (damaged_helmet.glb)
- Le casque était à l'envers → rotation +90° autour de X comme base
- `y_spin * base_x(+90°)` en quaternion (Rust) : `Quat::from_rotation_y(a) * Quat::from_rotation_x(FRAC_PI_2)`
- TypeScript : quaternion composé calculé analytiquement : `(S·cos(a/2), S·sin(a/2), -S·sin(a/2), S·cos(a/2))`

---

---

## Phase 3a — Shadow maps + Plugin system

**Réalisé :**

### Shadow maps (directional light, PCF 3×3)
- `LightUniforms` (80 bytes) : `view_proj mat4 + shadow_bias f32 + padding`
- `ShadowPass` : texture Depth32Float 2048×2048, pipeline depth-only, deux bind groups
  - `shadow_light_bind_group` : group 0 du shadow pass (LightUniforms, VERTEX)
  - `main_bind_group` : group 4 du PBR pass (LightUniforms + shadow_map + comparison sampler)
- `shadow_depth.wgsl` : vertex-only shader, lit seulement `@location(0) position`
- `pbr.wgsl` group 4 + `pcf_shadow()` 3×3 samples, `shadow_factor` appliqué à `direct`
- `RenderState` : `shadow_bg_layout` (group 4), pipeline layout étendu à 5 groupes
- Render loop two-pass : shadow depth → PBR main (natif + WASM)
- `build_light_uniforms()` : ortho [-10,10]³, lumière à `-light_dir * 20`
- Depth bias hardware : `constant=2, slope_scale=2.0` (anti-acne sans front-face trick)

### Plugin system (fondation Phase 3b)
- Trait `Plugin: 'static { fn name() → &str; fn build(&mut App) }`
- Struct `App { world: World, scheduler: Scheduler }` + `add_plugin<P: Plugin>()`
- Prévu pour enregistrer `PbrPlugin`, `ShadowPlugin`, `IblPlugin` en Phase 3b

**Décisions :**
- Pas de render graph déclaratif pour Phase 3a — passes séquentielles explicites
- `CompareFunction::LessEqual` dans le sampler de comparaison : 1.0 = lit, 0.0 = ombre
- Y-flip en WGSL : `ndc.xy * vec2(0.5, -0.5) + 0.5` (WebGPU NDC Y-up, UV Y-down)
- Frustum ortho fixe 20×20×50u centré à l'origine — suffisant pour le casque

---

---

## Phase 4 — GPU-driven pipeline + Hi-Z occlusion culling

**Réalisé :**

### Draw Indirect
- `DrawIndexedIndirectArgs` (20 bytes, bytemuck) mappé sur la spec WebGPU
- `entity_indirect_buf` : un slot par entité, `instance_count` mis à 0/1 par le GPU
- `build_entity_list()` + `build_batches()` : tri par mesh+matériau, upload matrices en instance buffer SoA
- `RenderPass::draw_indexed_indirect()` remplace les draw calls CPU

### Hi-Z pyramid (`HizPass`)
- Z-prepass via `hiz_depth.wgsl` : rendu de profondeur dans une texture `Depth32Float` full-res
- Compute `hiz_build.wgsl` : génère la mipchain R32Float (max-z reduction 2×2), 64×64 → 1×1
- `mip_count = log2(min(w,h)).floor()` (minimum 1)
- `hiz_full_view` : vue sur le mip 0 (profondeur full-res pour le sampling exact)

### Culling GPU (`CullPass` + `occlusion_cull.wgsl`)
- Frustum AABB (8 coins en clip space) + test Hi-Z (AABB projeté vs mip sélectionné)
- **Fix near-plane straddling** (2025-04-20) : `any_behind` conservatif → `depth_near = 0.0`, skip frustum XY cull. Corrigeait l'alternance visible/caché sur les casques dont l'AABB straddlait le near plane.
- `CullUniforms` : `view_proj`, `screen_size`, `entity_count`, `mip_levels`, `cull_enabled`
- `entity_indirect_buf` avec `COPY_SRC` pour le readback de métriques

### Tests GPU headless (`cull_integration.rs`)
- `try_gpu()` → skip gracefully si pas de GPU (CI-friendly)
- 9 tests couvrant : tout visible, tout derrière, frustum XY, Hi-Z depth, `any_behind` conservatif
- Textures Hi-Z synthétiques remplies à valeur constante pour déterminisme

### Démo native 3 scènes
- Orbit camera (drag LMB + scroll)
- Scène 1 Wall : mur + 1200 sphères + 6 témoins cyan
- Scène 2 Sieve : 10 piliers + 441 sphères
- Scène 3 Peekaboo : occulteur animé + 400 sphères
- Invariant de monotonie `debug_assert!` chaque frame : `hiz ≤ frustum ≤ total`
- Titre fenêtre : `Total | Frustum | Hi-Z drawn | Cull ON/OFF`

---

## Phase 5 — Post-processing (bloom + ACES + FXAA)

**Réalisé :**

### Pipeline HDR
- `HdrTarget` : texture `RGBA16Float` intermédiaire, rebuild au resize
- `pbr.wgsl` : suppression Reinhard — sort désormais en linéaire HDR
- `RenderState` : format cible pipeline PBR = `Rgba16Float` (plus swapchain)

### Bloom (`bloom.wgsl`)
- `fs_prefilter` : downsample ×2 avec Karis-weighted 4-tap + soft-knee threshold
  - `BloomParams { threshold, knee }` en uniform
  - Karis weight : `c / (1 + luma(c))` — supprime les fireflies
- `fs_blur_h` / `fs_blur_v` : gaussian 9-tap séparable, σ ≈ 3 texels
  - Poids : W0=0.0630 W1=0.0929 W2=0.1227 W3=0.1449 W4=0.1532
- 2 passes H+V ping-pong entre `bloom_a` et `bloom_b` (RGBA16Float half-res)

### Tone mapping + FXAA (`tonemap.wgsl`)
- ACES Narkowicz : `(x(ax+b))/(x(cx+d)+e)`, a=2.51 b=0.03 c=2.43 d=0.59 e=0.14
- Bloom additif : `hdr + bloom * bloom_strength`
- FXAA 3×3 luma neighbourhood, blend conditionnel (contrast > max(0.031, lMax·0.125))
- `linear_to_srgb()` : conversion piecewise (`c ≤ 0.0031308` → linéaire, sinon pow(1/2.4))

### `PostProcessPass`
- Chaîne complète en un `encode(encoder, swapchain_view)` : prefilter → H×2 → V×2 → tonemap
- `resize(device, hdr_view, w, h)` rebuild les bloom textures et tous les bind groups
- `update_bloom_params()` / `update_tonemap_params()` via `queue.write_buffer`

---

## Phase 3b — ECS Archetypes SoA + Rayon (2025-04-20)

**Contexte :** L'ECS utilisait déjà l'archetype SoA depuis Phase 2 (colonnes `Vec<T>` par type dans chaque archetype). Phase 3b ajoute la parallélisation Rayon et valide l'objectif de performance.

**Réalisé :**

### Nouvelles méthodes sur `World`

```rust
// Itérateur mutable série sur un type de composant
pub fn iter_mut<T: 'static>(&mut self) -> impl Iterator<Item = (Entity, &mut T)>

// Applique f en parallèle (Rayon natif / série WASM) sur toutes les T
pub fn for_each_mut<T, F>(&mut self, f: F)

// Idem mais uniquement dans les archetypes ne contenant PAS Excl
// → clé pour paralléliser les entités sans hiérarchie
pub fn for_each_without_mut<T, Excl, F>(&mut self, f: F)
```

### Optimisation `transform_system`

**Avant :** BFS séquentiel sur toutes les entités, `get_component_mut` par entité (O(1) HashMap).

**Après :**
- **Passe 1 (parallèle)** : `for_each_without_mut::<TransformComponent, HierarchyComponent>` → `world_matrix = local_matrix` sur toutes les entités sans hiérarchie. Rayon parallélise sur les cœurs disponibles.
- **Passe 2 (BFS séquentiel)** : uniquement pour les entités avec `HierarchyComponent`. No-op pour les scènes plates.

### Résultats benchmark — `cargo bench -p w3drs-ecs` (release, Rayon)

Machine : Windows 11, AMD/Intel desktop (2025-04-20)

| Entités | Temps médian | vs objectif |
|---|---|---|
| 1 000 | 23 µs | — |
| 10 000 | 51 µs | — |
| **100 000** | **~430 µs** | **< 2 ms ✅ (4.6× marge)** |

Commande de reproduction :
```bash
cargo bench -p w3drs-ecs
```
Résultats HTML dans `target/criterion/`.

### Dépendances ajoutées
- `rayon = "1"` — `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` dans `w3drs-ecs/Cargo.toml`
- `criterion = "0.5"` — `[dev-dependencies]`, bench `benches/transform.rs`

### Tests
- 36 tests ECS existants : tous verts
- `cargo check -p khronos-pbr-sample` : 0 warning

---

## Phase A — Parité rendu w3dts (PBR + IBL) — 2026-04

**Objectif roadmap** : [ROADMAP § Phase A](ROADMAP.md) ; ticket [Phase A — PBR + matériaux](tickets/phase-A-pbr-materiaux-gltf.md).

**Réalisé :**

- **`pbr.wgsl`** : recopie fonctionnelle des helpers w3dts (`DistributionGGX`, `GeometrySmith4`, `D_GGX_Anisotropic`, `V_SmithGGXCorrelated_Anisotropic`, `fresnelSchlick*`) et du flux `pbr_master_node` (TBN Gram–Schmidt + handedness, `roughness` 0.04–1, direct `(D·G·F)/(4 NdotV NdotL)` cutout `roughness ≥ 0.999`, IBL bent normal + `mix(R, N, r²)`, `MAX_REFLECTION_LOD = 4`). Clearcoat additif (extension w3drs) en GGX + Smith4. Flags `ibl_flags` / `ibl_diffuse_scale` sur le diffuse IBL.
- **`ibl.rs`** : intégrale LUT split-sum alignée w3dts `brdf.frag` (visibilité corrélée + poids) ; `BRDF_LUT_SAMPLES = 1024`.
- **Exemples** : `khronos-pbr-sample`, `w3drs-wasm`, **`hdr-ibl-skybox`** (debug env / IBL, args `--tier` / chemin `.hdr` comme `hdr-ibl-bench`).
- **CI** : pre-commit `cargo check` natif + wasm32.

**Clôture (2026-04-20)** : Phase A **terminée** pour le périmètre [checklist PBR](tickets/phase-a-pbr-checklist-w3dts.md) + [enregistrement des gates](tickets/phase-a-gates-record.md) (DamagedHelmet, natif + web). **Automatisé** vérifié : `cargo test -p w3drs-assets --test phase_a_fixture`, `cargo test -p w3drs-assets -p w3drs-renderer`, `cargo xtask check` — tous verts. **Suite hors ticket** : golden SSIM **optionnel** ; variantes **JSON/RON** et rigueur transmission (opaque + réfraction) : [Moyen terme — ticket A](tickets/phase-A-pbr-materiaux-gltf.md#moyen-terme--rigueur-pbr--prod) et [ROADMAP § Phase A — Poursuites](ROADMAP.md#phase-a--parité-rendu-moteur-pbr--matériaux--gltf).

**Suite (2026-04-20)** : `materials/default.json` + module `phase_a_viewer_config` : `ibl_diffuse_scale` et `tonemap` lus par `khronos-pbr-sample` (remplace des constantes Rust pour ces paramètres). Même schéma côté **web** : `parse_phase_a_viewer_config_str_or_default`, `W3drsEngine::applyPhaseAViewerConfigJson`, `www/public/phase-a/materials/default.json` chargé au boot dans `www/src/main.ts` (`npm run build:wasm` → `www/pkg/`).

**Suite (2026-04-21)** : extensions **KHR** (*emissive_strength*, *specular*, *transmission*, *volume*) lues côté `gltf` + shader ; **stratégie A1** (WGSL direct) vs **A2** (hors scope court) + **matrice natif / WASM** figées dans [phase-a-pbr-checklist-w3dts](tickets/phase-a-pbr-checklist-w3dts.md) ; [Moyen terme PBR/prod](tickets/phase-A-pbr-materiaux-gltf.md#moyen-terme--rigueur-pbr--prod) (transmission « vraie », sheen, JSON variantes) pour la suite.

**Suite (www — manifeste modèles)** : [`www/public/phase-a/viewer-manifest.json`](../www/public/phase-a/viewer-manifest.json) (ids alignés sur le manifeste `fixtures/`, chemins d’URL Vite) consommé par `www/src/main.ts` — `?m=`, **←/→** si plusieurs entrées ; second modèle léger **TextureTransform** sous `www/public/phase-a/glb/` (aligné `texture_transform_test`) en plus du gate *DamagedHelmet* (`.glb` lourds ailleurs = copie manuelle si besoin).

**Résolution textures IBL** (bake partagé natif / WASM, `ibl.rs` + `ibl_spec.rs`) : par défaut **`ibl_tier`: `max`** = irradiance **128²×6** (1 mip, `Rgba16Float`) · pré-filtré **512²×6** (**10** mips, `Rgba16Float`) · LUT BRDF **256²** (`Rg16Float`). Autres préréglages **`high` / `medium` / `low` / `min`** (tailles décroissantes) — voir [checklist Phase A](tickets/phase-a-pbr-checklist-w3dts.md) (section *Préréglages ibl_tier*). Bench : `cargo run -p hdr-ibl-bench --release -- --tier=…`.

**Mesures charge HDR** (`www/public/studio_small_03_2k.hdr`, **session 2026-04-23**, Windows, build **--release**) :

| Outil | parse (ms) | IBL bake (ms) | env_bind (ms) | total cœur (ms) | Remarque |
|-------|------------|---------------|---------------|-----------------|----------|
| `cargo run -p hdr-ibl-bench --release -- --tier=max` | 16.12 | 9818.54 | — | 9834.66 | Binaire headless, pas de bind group (`load_hdr` + `IblContext::from_hdr_with_spec`, tier **max**) |
| `khronos-pbr-sample` (stderr + log) | 19.7 | 9281.8 | 0.1 | 9301.6 | Intégré (chaîne env complète) ; l’écart IBL vs bench = charge machine / ordre d’exécution sur la même session |
| `www` + `w3drs-wasm` (console) — *mesure contributeur* | 39.93 | 120515.46 | 0.03 | 120555.42 | Même binaire `studio_small_03_2k` (~6,67 Mo) : `clientFetch+Buffer=37,47` ms, mur appel `load_hdr` = `clientWasmCallWallMs` **120559** ms. Le bake IBL côté WASM est **bien plus lent** qu’en natif sur la même plage d’environnements (navigateur : JS/WASM, thread unique pour le cœur CPU) — *ordre de grandeur* utile, pas chrono labo. |

**Grille `hdr-ibl-bench` par `ibl_tier`** (même fichier HDR, **session 2026-04-20**, Windows **--release**, une passe par tier) :

| `ibl_tier` | parse (ms) | IBL bake (ms) | core (ms) |
|------------|------------|---------------|-----------|
| `min` | 24.55 | 31.72 | 56.27 |
| `low` | 26.79 | 187.09 | 213.88 |
| `medium` | 17.27 | 671.68 | 688.95 |
| `high` | 19.04 | 4509.18 | 4528.22 |
| `max` | 16.08 | 9456.87 | 9472.95 |

- **Rejouer** le bench : `cargo run -p hdr-ibl-bench --release -- --tier=max` (chemin `.hdr` optionnel après les options ; `-t` / `--tier`). Le **natif** affiche `HDR (natif) …` sur **stderr** au boot. **WASM** : `ibl_tier` dans `phase-a/materials/default.json` + rechargement page ; console `[w3drs] HDR timing` / `w3drsHdrLoadTimings`. **Exemple fenêtré** : `cargo run -p hdr-ibl-skybox --release -- --tier=low` (mêmes args que le bench).

---

## Phase B — Graphe de rendu (jalon v0) — 2026-04

**Objectif** : [ROADMAP § Phase B](ROADMAP.md) ; ticket [Phase B — graphe & compute](tickets/phase-B-graphe-rendu-compute.md).

**Réalisé** :

- Schéma : [`docs/schemas/render-graph-v0.md`](schemas/render-graph-v0.md).
- Fixture : [`fixtures/phases/phase-b/`](../fixtures/phases/phase-b/) (`render_graph.json`, WGSL compute + raster minimal, `expected.md`, README).
- Crate **`w3drs-render-graph`** : `parse_render_graph_json` + validation + tests parse.
- **Natif** : `w3drs-renderer` → module **`render_graph_exec`** (`run_graph_v0_checksum` : passes du JSON, readback `Rgba16Float`, FNV-1a 64) ; test d’intégration **`phase_b_graph_exec`** (deux soumissions → même checksum ; skip si pas d’adaptateur GPU).
- **B.1 (registre GPU)** : [`RenderGraphGpuRegistry`](../crates/w3drs-renderer/src/render_graph_exec.rs) — textures / buffers nommés depuis le JSON, `resize_texture_2d`, `run_graph_v0_checksum_with_registry` ; tests étendus [`phase_b_graph_exec`](../crates/w3drs-renderer/tests/phase_b_graph_exec.rs).
- **B.2 (pré-barrières)** : `validate_exec_v0` étendu — `texture_reads` / `storage_writes` sur les passes `compute` (optionnel JSON), contrôle des usages `render_attachment` / `texture_binding` / `storage` vs références des passes, interdiction des doublons dans `color_targets`, `depth_target` + formats depth ; `pass_ids_in_order_v0` ; erreurs mappées côté `RenderGraphExecError` (natif). **Exécuteur** : `raster_mesh` attache `depth_target` (fixture `scene_depth` + sync `www/public/phase-b/`). Schéma : [render-graph-v0.md](schemas/render-graph-v0.md) ; ticket [Phase B](tickets/phase-B-graphe-rendu-compute.md).

**Suite (validation)** : `validate_render_graph_exec_v0` (sans GPU) + tests d’erreur `RenderGraphExecError` ; `Rgba8Unorm` autorisé pour textures non-readback ; readback checksum = **`Rgba16Float`** uniquement (`InvalidReadbackFormat`).

**Suite (WASM / partage)** : `validate_exec_v0` déplacé dans **`w3drs-render-graph`** (sans `wgpu`) ; export **`w3drsValidateRenderGraphV0`** dans le paquet `www/pkg` ; `w3drs-renderer` réexporte `validate_exec_v0` + `RenderGraphValidateError` (natif).

**Exemple CLI** : `cargo run -p phase-b-graph --release` — charge `fixtures/phases/phase-b/render_graph.json` (+ `shaders/`), valide, affiche le **checksum** (stderr).

**Web** : `www/public/phase-b/` (JSON + WGSL copiés du fixture) + `main.ts` → `w3drsValidateRenderGraphV0` au chargement (smoke aligné natif).

**Suite** : bindings / barrières génériques, fusion avec le viewer PBR, exécuteur GPU WASM. **Plan détaillé (jalons B.1–B.6)** : [Phase B — *Plan d’exécution*](tickets/phase-B-graphe-rendu-compute.md#plan-dexécution--exécuteur-complet--wasm-cible-w3dts) ; [schéma render-graph v0 — feuille de route](schemas/render-graph-v0.md#feuille-de-route--exécuteur-complet-parité-moteur-objectif-w3dts).

---

## Documentation — port 1:1, gates Phase A, plan Phase B (2026-04-24)

**Contexte** : cadrer l’**intention produit** d’un **port 1:1** w3dts → w3drs (équivalence **fonctionnelle** par domaine, exceptions d’implémentation explicites) et consigner la **procédure** pour les gates Phase A (désormais **clôturés** — voir [phase-a-gates-record.md](tickets/phase-a-gates-record.md)) et la suite Phase B.

**Réalisé (doc + règles)** :

- [ROADMAP](ROADMAP.md) : section *Port 1:1 — définition et exceptions acceptables* (table moteur / `.w3db` / éditeur / périmètre) ; objectif d’accroche réécrit en tête de document.
- [tickets/README.md](tickets/README.md) : lien vers cette section pour les revues *parité*.
- [`.cursor/rules/w3drs-w3dts-migration.mdc`](../.cursor/rules/w3drs-w3dts-migration.mdc) : règle alignée sur l’intention 1:1.
- [phase-a-gates-record.md](tickets/phase-a-gates-record.md) : modèle d’**enregistrement** pour les *gates* visuels (natif + web) ; [phase-a-pbr-checklist-w3dts.md](tickets/phase-a-pbr-checklist-w3dts.md) : renvoi *Enregistrement DOD* ; [phase-A-pbr-materiaux-gltf.md](tickets/phase-A-pbr-materiaux-gltf.md) : DOD liée aux gates.
- [phase-B-graphe-rendu-compute.md](tickets/phase-B-graphe-rendu-compute.md) : *Plan d’exécution* (B.0–B.6) + statut ; [render-graph-v0.md](schemas/render-graph-v0.md) : *Feuille de route* exécuteur complet.
- [ROADMAP](ROADMAP.md) *Barre de progression* : note de **révision 2026-04-24** (mise à jour textuelle sans recalcul de pourcentage).

**Suite** : ~~emplir phase-a-gates-record~~ **fait** (clôture Phase A 2026-04-20) ; implémenter jalons Phase B (B.1→) côté code.

---

## À venir

- **Phase A** : ~~clôture~~ **terminée** (2026-04-20) — [phase-a-gates-record.md](tickets/phase-a-gates-record.md), [ticket A](tickets/phase-A-pbr-materiaux-gltf.md) **Terminée**
- **Phase B** : [plan B.1–B.6](tickets/phase-B-graphe-rendu-compute.md#plan-dexécution--exécuteur-complet--wasm-cible-w3dts) (registre, barrières, viewer, WASM, ECS)
- **ROADMAP** : Phase K (éditeur / workspace) — voir [ROADMAP.md](ROADMAP.md)
