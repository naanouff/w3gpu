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

## À venir

- **Phase 3b** : ECS Archetypes SoA + Rayon (migration HashMap → stockage contigu)
- **Phase 4** : GPU-driven (Draw Indirect, Hi-Z occlusion culling)
- **Phase 5** : Post-processing (bloom, ACES tone mapping, FXAA)
