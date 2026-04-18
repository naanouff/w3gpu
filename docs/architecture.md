# Architecture

## Structure du workspace

```
crates/
  w3gpu-math/       Glam wrappers, AABB, BoundingSphere, Frustum
  w3gpu-ecs/        World, Scheduler, composants (Transform, Camera, Lights…)
  w3gpu-assets/     Mesh, Material, Vertex, glTF loader, HDR loader, primitives
  w3gpu-renderer/   wgpu context, PBR pipeline, IBL, WGSL shaders, AssetRegistry
  w3gpu-wasm/       wasm-bindgen glue — API JS/TS publique
examples/
  native-triangle/  Client desktop (winit + pollster)
www/                Vite + TypeScript, consomme le package WASM
xtask/              cargo xtask runner (www, client, check, setup-hooks)
docs/               Bibliothèque documentaire (ce dossier)
```

---

## Crate dependency graph

```
w3gpu-math
    └── w3gpu-ecs
    └── w3gpu-assets
            └── w3gpu-renderer
                    └── w3gpu-wasm
                    └── native-triangle (example)
```

`w3gpu-ecs` ne dépend pas de `w3gpu-renderer` — les systèmes ECS sont testables sans GPU.

---

## ECS (état actuel — HashMap)

### Stockage

```rust
// w3gpu-ecs/src/world.rs
struct ComponentStorage<T> = HashMap<Entity, T>
World { stores: HashMap<TypeId, Box<dyn AnyStorage>> }
```

- O(1) get/insert par entity
- Pas cache-friendly (données dispersées en mémoire)
- Fonctionnel, ~400 lignes

### Composants existants

| Composant | Fichier | Rôle |
|---|---|---|
| `TransformComponent` | ecs/components/transform.rs | TRS + world matrix |
| `CameraComponent` | ecs/components/camera.rs | View/Proj matrices, frustum |
| `RenderableComponent` | ecs/components/renderable.rs | mesh_id + material_id |
| `CulledComponent` | ecs/components/culled.rs | Tag zero-size (frustum culling) |

### Systèmes

```
transform_system      → matrices locales → monde (itératif)
camera_system         → view/proj matrices
frustum_culling_system → ajoute/retire CulledComponent
```

### ECS cible (Phase 3b) — Archetypes SoA

```rust
// Archetype = ensemble unique de TypeIds, stockage contigu par colonne
struct Archetype {
    component_vecs: HashMap<TypeId, Box<dyn ErasedVec>>,
    entities: Vec<Entity>,
}
struct World {
    archetypes: Vec<Archetype>,
    entity_location: HashMap<Entity, (ArchetypeId, usize)>,
}
```

Cible : 100k entities, `transform_system` < 2ms. Compatible Rayon.

---

## Renderer

### Contexte GPU

```rust
// w3gpu-renderer/src/gpu_context.rs
struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_format: wgpu::TextureFormat,
    depth_texture: wgpu::Texture,     // Depth32Float
    depth_view: wgpu::TextureView,
}
```

Cible wasm32 : `Backends::BROWSER_WEBGPU`
Cible native : `Backends::all()` (DX12 / Metal / Vulkan)

### Pipeline de rendu (état actuel)

```
ShadowPass (group 0 = light VP, group 1 = object)   ← à implémenter (Phase 3a)
    ↓ shadow_map: Depth32Float 2048×2048
MainPass   (group 0 = frame, 1 = object, 2 = material, 3 = IBL, 4 = shadow)
    ↓ surface présentation
```

### AssetRegistry

Registre des ressources GPU avec IDs opaques :

```rust
upload_mesh(mesh, device, queue)                  → mesh_id: u32
upload_texture_rgba8(data, w, h, srgb, ...)       → tex_id: u32
upload_material(material, textures, ...)          → mat_id: u32
```

Fallbacks automatiques : 1×1 textures (white, flat-normal, default-mr, black) créés à l'init.
Material id=0 = matériau par défaut (toujours présent).

### IBL (implémenté)

Précomputation CPU depuis une image HDR équirectangulaire :

| Ressource | Format | Taille |
|---|---|---|
| Irradiance cubemap | rgba16float | 32×32×6 |
| Prefiltered env cubemap | rgba16float | 128×128×6×5mips |
| BRDF LUT | rg16float | 256×256 |

API : `IblContext::from_hdr(hdr, device, queue, layout)` ou `IblContext::new_default(...)`.

---

## Plugin system (Phase 3a)

Trait central pour l'extensibilité :

```rust
pub trait Plugin: 'static {
    fn build(&self, app: &mut App);
}
pub struct App {
    world: World,
    scheduler: Scheduler,
    // ...
}
impl App {
    pub fn add_plugin<P: Plugin>(&mut self, p: P) -> &mut Self
}
```

Plugins prévus : `PbrPlugin`, `IblPlugin`, `ShadowPlugin`, `FxaaPlugin`.

---

## GPU-Driven Pipeline (Phase 4)

Objectif : réduire les draw calls CPU-bound.

```
compute pass → frustum culling + Hi-Z → IndirectBuffer
render pass  → multi_draw_indexed_indirect(IndirectBuffer)
```

Cible : 100k draw calls < 8ms.

---

## Targets de build

| Target | Backend wgpu | Features Cargo |
|---|---|---|
| `wasm32-unknown-unknown` | `webgpu` | `w3gpu-wasm` |
| Windows native | `dx12` | `native-triangle` |
| macOS native | `metal` | `native-triangle` |
| Linux native | `vulkan-portability` | `native-triangle` |
