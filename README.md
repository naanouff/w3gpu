# w3drs

A 3D engine written in Rust, compiled to WebAssembly and running on WebGPU in the browser.

## Features

- **ECS** — Archetype SoA storage, parallel transform via Rayon (100k entities ~430 µs), iterative hierarchy BFS
- **WebGPU** — Native WebGPU backend via `wgpu`, no WebGL fallback
- **WASM** — Full browser target via `wasm-pack`, callable from TypeScript
- **Cross-platform** — Also runs natively on Windows (DX12), macOS (Metal), Linux (Vulkan)
- **PBR rendering** — Cook-Torrance GGX/Smith BRDF, IBL (irradiance + prefiltered env + BRDF LUT), directional light + PCF shadow maps
- **GPU-driven pipeline** — Draw Indirect, Hi-Z occlusion culling (compute), frustum culling
- **Post-processing** — Bloom (Karis prefilter + separable gaussian), ACES tone mapping, FXAA
- **glTF loader** — Load `.glb` files at runtime (browser via `fetch`, native via filesystem)

## Workspace structure

```
crates/
  w3drs-math/       # Math types: Vec3, Mat4, Quat, AABB, BoundingSphere, Frustum
  w3drs-ecs/        # World, Scheduler, archetype SoA storage, Rayon parallel iter
  w3drs-assets/     # Mesh, Material, Vertex, glTF loader, HDR loader, primitives
  w3drs-renderer/   # wgpu context, PBR + IBL + shadows + post-processing, Hi-Z cull
  w3drs-wasm/       # wasm-bindgen glue — public JS/TS API
examples/
  native-triangle/  # Desktop client (winit) — 3-scene Hi-Z validation demo
www/                # Vite project consuming the WASM package
  public/           # Static assets tracked via Git LFS (*.glb, *.gltf, *.bin, *.hdr)
xtask/              # cargo xtask runner
docs/               # Architecture, shaders, API reference, implementation journal
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
cargo test -p w3drs-math -p w3drs-ecs -p w3drs-assets

# Lint
cargo clippy -- -D warnings
```

Open `http://localhost:5173` in a browser that supports WebGPU (Chrome 113+, Edge 113+).

## TypeScript API

```typescript
import init, { W3drsEngine } from './pkg/w3drs_wasm.js';

await init();
const engine = await W3drsEngine.create('my-canvas');

// Load an equirectangular HDR for image-based lighting (optional, call before load_gltf)
const hdr = new Uint8Array(await (await fetch('/env.hdr')).arrayBuffer());
engine.load_hdr(hdr); // precomputes irradiance 32×32, prefiltered 128×128×5mips, BRDF LUT 256×256

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
- [x] Phase 1 — ECS, PBR, glTF loader, texture sampling (albedo · normal · MR · emissive)
- [x] Phase 2 — IBL (CPU precompute: irradiance 32×32, prefiltered 128×128×5mips, BRDF LUT 256×256)
- [x] Phase 3a — Shadow maps (PCF 3×3), plugin system
- [x] Phase 3b — ECS archetype SoA + Rayon parallel transform (100k entities < 2ms)
- [x] Phase 4 — GPU-driven pipeline: Draw Indirect, Hi-Z pyramid, occlusion culling compute
- [x] Phase 5 — Post-processing: bloom, ACES tone mapping, FXAA
- [ ] Phase 6 — Editor (Design / Debug / Ship modes)
- [ ] Phase 7 — SaaS bridge + Cloud compute

See [docs/journal.md](docs/journal.md) for implementation details and decisions.

## License

MIT
