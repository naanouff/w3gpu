# Journal d'implémentation

## Phase 0 — Scaffold

**Réalisé :**
- Workspace Cargo multi-crates (`w3gpu-math`, `w3gpu-ecs`, `w3gpu-assets`, `w3gpu-renderer`, `w3gpu-wasm`)
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
- `W3gpuEngine::tick(dt)` → render pass → submit
- `requestAnimationFrame` TypeScript
- Client natif `native-triangle` avec winit

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

### HDR Loader (`w3gpu-assets/src/hdr_loader.rs`)
- `load_hdr_from_bytes(bytes) → HdrImage { pixels: Vec<[f32;3]>, width, height }`
- Utilise `image::load_from_memory_with_format` avec `ImageFormat::Hdr`

### Précomputation CPU (`w3gpu-renderer/src/ibl.rs`)
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
- `engine.load_hdr(bytes: Uint8Array) → void` — appelé avant `load_gltf`
- `www/src/main.ts` : fetch HDR, appel `load_hdr`, puis load_gltf

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

### Résultats benchmark — `cargo bench -p w3gpu-ecs` (release, Rayon)

Machine : Windows 11, AMD/Intel desktop (2025-04-20)

| Entités | Temps médian | vs objectif |
|---|---|---|
| 1 000 | 23 µs | — |
| 10 000 | 51 µs | — |
| **100 000** | **~430 µs** | **< 2 ms ✅ (4.6× marge)** |

Commande de reproduction :
```bash
cargo bench -p w3gpu-ecs
```
Résultats HTML dans `target/criterion/`.

### Dépendances ajoutées
- `rayon = "1"` — `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` dans `w3gpu-ecs/Cargo.toml`
- `criterion = "0.5"` — `[dev-dependencies]`, bench `benches/transform.rs`

### Tests
- 36 tests ECS existants : tous verts
- `cargo check -p native-triangle` : 0 warning

---

## À venir

- **Phase 6** : Éditeur multi-mode (Design / Debug / Ship)
- **Phase 7** : SaaS bridge + Cloud compute
