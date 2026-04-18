# w3gpu

A 3D engine written in Rust, compiled to WebAssembly and running on WebGPU in the browser.

## Features

- **ECS** — Custom entity-component system with sparse storage and iterative transform hierarchy
- **WebGPU** — Native WebGPU backend via `wgpu`, no WebGL fallback
- **WASM** — Full browser target via `wasm-pack`, callable from TypeScript
- **Cross-platform** — Also runs natively on Windows (DX12), macOS (Metal), Linux (Vulkan)
- **PBR rendering** *(in progress)* — Physically-based materials, shadow maps, IBL
- **Render graph** *(in progress)* — Declarative pass-based pipeline

## Workspace structure

```
crates/
  w3gpu-math/       # Math types: Vec3, Mat4, Quat, AABB, BoundingSphere, Frustum
  w3gpu-ecs/        # World, Scheduler, components (Transform, Camera, Lights...)
  w3gpu-assets/     # Mesh, Material, vertex layout, primitive generators
  w3gpu-renderer/   # wgpu context, render graph, WGSL shaders
  w3gpu-wasm/       # wasm-bindgen glue — public JS/TS API
examples/
  native-triangle/  # Desktop smoke test (winit)
www/                # Vite project consuming the WASM package
```

## Getting started

### Prerequisites

- [Rust](https://rustup.rs) (stable)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)
- [Node.js](https://nodejs.org) ≥ 18

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

### Quick start

```bash
cargo xtask www      # build WASM + start Vite dev server (http://localhost:5173)
cargo xtask client   # build + run the native desktop client
```

Both commands handle all build steps automatically. `cargo xtask www` also runs `npm install` if `node_modules` is missing.

### Manual commands

```bash
# Desktop only
cargo run -p native-triangle

# Browser only
cd www && npm install && npm run dev

# Build WASM only
wasm-pack build crates/w3gpu-wasm --target web --out-dir www/pkg
```

Open `http://localhost:5173` in a browser that supports WebGPU (Chrome 113+, Edge 113+).

## TypeScript API

```typescript
import init, { W3gpuEngine } from './pkg/w3gpu_wasm.js';

await init();
const engine = await W3gpuEngine.create('my-canvas');

const entity = engine.create_entity();
engine.tick(deltaTime);
```

## Development

```bash
# Check all crates (no GPU required)
cargo check

# Unit tests (math, ECS, assets — no GPU)
cargo test -p w3gpu-math -p w3gpu-ecs -p w3gpu-assets

# Check WASM target
cargo check -p w3gpu-wasm --target wasm32-unknown-unknown
```

## Roadmap

- [x] Phase 0 — Workspace scaffold, triangle on screen (native + WASM)
- [ ] Phase 1 — ECS-driven cube with perspective camera
- [ ] Phase 2 — PBR lighting, shadow maps, glTF loader
- [ ] Phase 3 — Render graph, post-processing (bloom, tone mapping, FXAA)
- [ ] Phase 4 — GPU instancing, LOD, occlusion culling

## License

MIT
