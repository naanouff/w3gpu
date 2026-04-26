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

## Current architecture vs documented target

The **code today** is a focused **Rust + wgpu + WASM** runtime: archetype ECS, PBR with IBL and shadows, a GPU-driven path (indirect draw, Hi-Z), post-processing, and a **glTF** path that is still largely **code-centric** (plus the **`khronos-pbr-sample`** native viewer and the **`www/`** Vite shell). The **documented target** (see [`docs/architecture.md`](docs/architecture.md) and [`docs/tickets/README.md`](docs/tickets/README.md)) adds: **data-first** scene and pipeline description (render/shader/terrain/script/particle graphs, physics serialization), a **`.w3db`** project package with streaming, a **native editor** around a **workspace**, a **multi-target compiler** (native exe, Node/React shell, or static “Unity-style” page), **third-party plugins** as **DLL/dylib/so** on desktop and **separate `.wasm` modules`** on the web, broader **import formats** (OBJ, STEP/AP242, point clouds, Gaussian splats) under clear priority rules, and **CI-grade** testing (coverage, native + browser E2E). Each phase ticket carries an **“Écart architecture (existant → cite)”** subsection so work is explicitly tied to closing those gaps.

## Workspace structure

```
crates/
  w3drs-math/       # Math types: Vec3, Mat4, Quat, AABB, BoundingSphere, Frustum
  w3drs-ecs/        # World, Scheduler, archetype SoA storage, Rayon parallel iter
  w3drs-assets/     # Mesh, Material, Vertex, glTF loader, HDR loader, primitives
  w3drs-renderer/   # wgpu context, PBR + IBL + shadows + post-processing, Hi-Z cull
  w3drs-render-graph/  # Phase B: JSON parse + validate_exec_v0 (no wgpu; shared native + WASM)
  w3drs-wasm/       # wasm-bindgen glue — public JS/TS API
examples/
  khronos-pbr-sample/  # Desktop viewer (winit) — seven Phase A GLB + orbit + IBL
  phase-b-graph/        # Headless: render graph JSON + checksum (Phase B fixture)
editor/             # w3d-editor: shell auteur natif (egui) — coquille phase-k
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
cargo xtask client   # build + run the native PBR viewer (khronos-pbr-sample)
cargo xtask editor   # build + run the w3d-editor shell (egui, fixture phase-k)
```

Both commands handle all build steps automatically. `cargo xtask www` also runs `npm install` if `node_modules` is missing. On **Windows**, `xtask` appelle **`npm.cmd`** (évite le shim `npm.ps1` bloqué par la politique d’exécution PowerShell). TypeScript (Vitest) dans `www/` : `cd www && npm test` — voir [Testing policy](CONTRIBUTING.md#testing-policy) dans [CONTRIBUTING.md](CONTRIBUTING.md).

#### Windows PowerShell — « Impossible de charger … npm.ps1 »

Si `npm run dev` échoue avec *l’exécution de scripts est désactivée* :

- Depuis la racine du dépôt **`w3drs/`** : **`cargo xtask www`** (recommandé).
- Ou depuis **`w3drs/www/`** : **`npm.cmd run dev`** (ou **`npm.cmd run build:wasm`**) au lieu de `npm run …`.
- Alternative : ouvrir **cmd.exe** ou **Git Bash**, ou assouplir la politique pour l’utilisateur : `Set-ExecutionPolicy -Scope CurrentUser RemoteSigned` (décision locale à votre poste).

The native viewer (`khronos-pbr-sample`) cycles seven reference GLBs from the repo (DamagedHelmet + Phase A fixtures) with **← / →**, orbit camera, and the default HDR `www/public/studio_small_03_2k.hdr` for IBL.

```bash
cargo run -p khronos-pbr-sample --release
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
engine.load_hdr(hdr); // → HdrLoadStats (ms: parse, ibl, env bind) — irradiance / prefilter / BRDF LUT

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

### Fondations (livré)

- [x] Phase 0 — Workspace scaffold, triangle on screen (native + WASM)
- [x] Phase 1 — ECS, PBR, glTF loader, texture sampling (albedo · normal · MR · emissive)
- [x] Phase 2 — IBL (CPU precompute: irradiance 32×32, prefiltered 128×128×5mips, BRDF LUT 256×256)
- [x] Phase 3a — Shadow maps (PCF 3×3), plugin system
- [x] Phase 3b — ECS archetype SoA + Rayon parallel transform (100k entities < 2ms)
- [x] Phase 4 — GPU-driven pipeline: Draw Indirect, Hi-Z pyramid, occlusion culling compute
- [x] Phase 5 — Post-processing: bloom, ACES tone mapping, FXAA

### Cible « prod » — parité fonctionnelle avec le concept **w3dts** (Rust)

L’objectif produit est de porter le **périmètre runtime** défini par le moteur TypeScript **w3dts** vers **w3drs** : rendu avancé, format de scène / streaming, animation, physique, terrain, particules, audio, input, réseau, outillage — avec priorités et critères de done détaillés dans **[docs/ROADMAP.md](docs/ROADMAP.md)** (phases A → L).

Les entrées historiques ci-dessous restent des jalons haut niveau :

- [ ] Phase 6 — Éditeur & expérience développeur (voir phase **K** du ROADMAP aligné w3dts)
- [ ] Phase 7 — SaaS / cloud compute (voir phase **L** et extensions post-parité)

See [docs/ROADMAP.md](docs/ROADMAP.md) for the full w3dts-aligned plan, and [docs/journal.md](docs/journal.md) for implementation details and decisions.

## License

MIT
