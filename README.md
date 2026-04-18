# w3gpu

A 3D engine written in Rust, compiled to WebAssembly and running on WebGPU in the browser.

## Features

- **ECS** — Custom entity-component system with sparse storage and iterative transform hierarchy
- **WebGPU** — Native WebGPU backend via `wgpu`, no WebGL fallback
- **WASM** — Full browser target via `wasm-pack`, callable from TypeScript
- **Cross-platform** — Also runs natively on Windows (DX12), macOS (Metal), Linux (Vulkan)
- **PBR rendering** — Cook-Torrance GGX/Smith BRDF, directional light, per-material albedo/metallic/roughness/emissive
- **glTF loader** — Load `.glb` files at runtime (browser via `fetch`, native via filesystem)
- **Depth buffer** — Correct occlusion on all targets
- **Render graph** *(in progress)* — Declarative pass-based pipeline, shadow maps, IBL

## Workspace structure

```
crates/
  w3gpu-math/       # Math types: Vec3, Mat4, Quat, AABB, BoundingSphere, Frustum
  w3gpu-ecs/        # World, Scheduler, components (Transform, Camera, Lights...)
  w3gpu-assets/     # Mesh, Material, Vertex, glTF loader, primitive generators
  w3gpu-renderer/   # wgpu context, PBR pipeline, WGSL shaders, asset registry
  w3gpu-wasm/       # wasm-bindgen glue — public JS/TS API
examples/
  native-triangle/  # Desktop client (winit) — loads www/public/*.glb by default
www/                # Vite project consuming the WASM package
  public/           # Static assets tracked via Git LFS (*.glb, *.gltf, *.bin)
xtask/              # cargo xtask runner
```

## Getting started

### Prerequisites

- [Rust](https://rustup.rs) stable
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)
- [Node.js](https://nodejs.org) ≥ 18
- [Git LFS](https://git-lfs.com) (for binary assets in `www/public/`)

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
git lfs install
git lfs pull          # download tracked assets after cloning
```

### Quick start

```bash
cargo xtask www      # build WASM + start Vite dev server (http://localhost:5173)
cargo xtask client   # build + run the native desktop client
```

Both commands handle all build steps automatically. `cargo xtask www` also runs `npm install` if `node_modules` is missing.

The native client loads `www/public/damaged_helmet_source_glb.glb` by default. Pass a custom path as the first argument to the binary:

```bash
target\release\native-triangle.exe path\to\model.glb
```

### First-time setup (pre-commit hook)

```bash
cargo xtask setup-hooks   # installs .githooks/pre-commit into .git/hooks
```

The hook runs `cargo check` on native and `wasm32-unknown-unknown` targets before every commit. See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

### Manual commands

```bash
# Check native + wasm32 (same as pre-commit)
cargo xtask check

# Unit tests (no GPU required)
cargo test -p w3gpu-math -p w3gpu-ecs -p w3gpu-assets

# Lint
cargo clippy -- -D warnings
```

Open `http://localhost:5173` in a browser that supports WebGPU (Chrome 113+, Edge 113+).

## TypeScript API

```typescript
import init, { W3gpuEngine } from './pkg/w3gpu_wasm.js';

await init();
const engine = await W3gpuEngine.create('my-canvas');

// Load a GLB at runtime
const bytes = new Uint8Array(await (await fetch('/model.glb')).arrayBuffer());
const ids = engine.load_gltf(bytes); // [mesh_id0, mat_id0, mesh_id1, mat_id1, ...]

// Or use built-in primitives with custom PBR materials
const meshId = engine.upload_cube_mesh();
const matId  = engine.upload_material(1.0, 0.4, 0.1, 1.0,  0.05, 0.4,  0, 0, 0);
                                    // r    g    b    a     metal rough  emissive

const entity = engine.create_entity();
engine.set_mesh_renderer(entity, meshId, matId);
engine.set_transform(entity, 0, 0, 0,  0, 0, 0, 1,  1, 1, 1);
//                           pos        quat       scale

engine.tick(deltaTime);
```

## Assets

Binary assets (`*.glb`, `*.gltf`, `*.bin`) stored in `www/public/` are tracked via **Git LFS**. Run `git lfs pull` after cloning to download them. To add a new asset:

```bash
# LFS tracking is already configured in .gitattributes
# Just add the file normally — git will handle the rest
git add www/public/my-model.glb
git commit -m "assets: add my-model.glb"
```

## Roadmap

- [x] Phase 0 — Workspace scaffold, triangle on screen (native + WASM)
- [x] Phase 1 — ECS-driven cube with perspective camera and frustum culling
- [x] Phase 2 — PBR lighting, depth buffer, material system, glTF loader, texture sampling (albedo · normal · metallic-roughness · emissive)
- [ ] Phase 3 — Render graph, shadow maps, IBL, post-processing (bloom, tone mapping, FXAA)
- [ ] Phase 4 — GPU instancing, LOD, occlusion culling (HZB)
- [ ] Phase 5 — Native ray tracing (ray queries via wgpu, full RT pipeline via Vulkan/DXR)

## License

MIT
