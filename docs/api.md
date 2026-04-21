# API publique

## TypeScript / WASM (`W3drsEngine`)

```typescript
import init, { W3drsEngine } from './pkg/w3drs_wasm.js';

await init();
const engine = await W3drsEngine.create('canvas-id');

// Version
W3drsEngine.version(): string

// Entités
engine.create_entity(): number
engine.destroy_entity(entity: number): void

// Composants
engine.set_transform(entity, px, py, pz, qx, qy, qz, qw, sx, sy, sz): void
engine.set_mesh_renderer(entity, mesh_id, material_id): void
engine.add_camera(entity, fov_degrees, aspect, near, far): void

// Assets — meshes
engine.upload_cube_mesh(): number   // → mesh_id

// Assets — matériaux (sans texture)
engine.upload_material(r, g, b, a, metallic, roughness, er, eg, eb): number   // → mat_id

// Assets — GLB (avec textures)
// Retourne [mesh_id0, mat_id0, mesh_id1, mat_id1, ...]
engine.load_gltf(bytes: Uint8Array): number[]

// Assets — IBL (appeler avant load_gltf pour meilleur rendu)
engine.load_hdr(bytes: Uint8Array): void

// Boucle de rendu
engine.tick(delta_time: number): void   // dt en secondes
engine.resize(width: number, height: number): void
```

### Exemple complet

```typescript
await init();
const engine = await W3drsEngine.create('my-canvas');

// IBL optionnel
const hdr = new Uint8Array(await (await fetch('/env.hdr')).arrayBuffer());
engine.load_hdr(hdr);

// GLB
const glb = new Uint8Array(await (await fetch('/model.glb')).arrayBuffer());
const ids = engine.load_gltf(glb);  // [mesh0, mat0, mesh1, mat1, ...]

// Caméra
const cam = engine.create_entity();
engine.add_camera(cam, 60, canvas.width / canvas.height, 0.1, 1000);
engine.set_transform(cam, 0, 0, 3,  0, 0, 0, 1,  1, 1, 1);

// Entités scène
const entities = [];
for (let i = 0; i + 1 < ids.length; i += 2) {
  const e = engine.create_entity();
  engine.set_mesh_renderer(e, ids[i], ids[i+1]);
  engine.set_transform(e, 0, 0, 0,  0, 0, 0, 1,  1, 1, 1);
  entities.push(e);
}

// Boucle
let prev = performance.now(), angle = 0;
function frame() {
  const dt = (performance.now() - prev) / 1000; prev = performance.now();
  angle += dt * 0.4;
  const S = Math.SQRT1_2, ha = angle / 2;
  for (const e of entities)
    engine.set_transform(e, 0, 0, 0,  S*Math.cos(ha), S*Math.sin(ha), -S*Math.sin(ha), S*Math.cos(ha),  1, 1, 1);
  engine.tick(dt);
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);
```

---

## Rust interne — crates publics

### `w3drs-assets`

```rust
// Chargement glTF
load_from_bytes(bytes: &[u8]) -> Result<Vec<GltfPrimitive>, GltfError>

struct GltfPrimitive {
    mesh: Mesh,
    material: Material,
    albedo_image: Option<RgbaImage>,
    normal_image: Option<RgbaImage>,
    metallic_roughness_image: Option<RgbaImage>,
    emissive_image: Option<RgbaImage>,
}

// Chargement HDR
load_hdr_from_bytes(bytes: &[u8]) -> Result<HdrImage, HdrError>

struct HdrImage { pixels: Vec<[f32; 3]>, width: u32, height: u32 }

// Primitives géométriques
primitives::cube() -> Mesh
primitives::sphere(subdivisions: u32) -> Mesh   // à venir
```

### `w3drs-renderer`

```rust
// Contexte GPU
GpuContext::new(instance, surface, width, height) -> Result<GpuContext, EngineError>
context.resize(width, height)

// Pipeline + layouts
RenderState::new(device, surface_format) -> RenderState
// Champs publics : pipeline, frame/object/material/ibl/shadow_bg_layout,
//                 frame_uniform_buffer, frame_bind_group,
//                 object_uniform_buffer, object_bind_group

// Registre d'assets GPU
AssetRegistry::new(device, queue) -> AssetRegistry
registry.upload_mesh(mesh, device, queue) -> u32
registry.upload_texture_rgba8(data, w, h, srgb, device, queue) -> u32
registry.upload_material(material, MaterialTextures, device, layout) -> u32
registry.get_mesh(id) -> Option<&GpuMesh>
registry.get_material(id) -> Option<&GpuMaterial>

// IBL
IblContext::new_default(device, queue, layout) -> IblContext
IblContext::from_hdr(hdr, device, queue, layout) -> IblContext
// Champ public : bind_group

// Shadow (Phase 3a)
ShadowPass::new(device, shadow_bg_layout, object_bg_layout) -> ShadowPass
// Champs publics : depth_pipeline, shadow_view, main_bind_group, light_uniform_buffer
```

### `w3drs-ecs`

```rust
// Monde
World::new() -> World
world.create_entity() -> u32
world.destroy_entity(entity: u32)
world.add_component<T: 'static>(entity, component: T)
world.get_component<T: 'static>(entity) -> Option<&T>
world.get_component_mut<T: 'static>(entity) -> Option<&mut T>
world.has_component<T: 'static>(entity) -> bool
world.query_entities<T: 'static>() -> Vec<u32>

// Scheduler
Scheduler::new() -> Scheduler
scheduler.add_system(fn(&mut World, f32, f32)) -> &mut Scheduler
scheduler.run(world, delta_time, total_time)
```

---

## Commands cargo xtask

```bash
cargo xtask www       # wasm-pack build + npm install + vite dev
cargo xtask client    # cargo build --release + run khronos-pbr-sample
cargo xtask check     # cargo check native + wasm32
cargo xtask setup-hooks  # installe .githooks/pre-commit → .git/hooks/
```
