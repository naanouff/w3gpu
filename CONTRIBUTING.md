# w3drs — Development Guidelines

## Core Principles

| Principle | Description |
|-----------|-------------|
| **DRY**   | Extract common logic into reusable functions/traits |
| **SOLID** | Single responsibility, open/closed, dependency inversion |
| **KISS**  | Prefer simple, readable solutions over clever ones |
| **YAGNI** | Implement features when they are actually needed |

---

## Workspace Structure

```
w3drs/
├── crates/
│   ├── w3drs-math/          # glam wrappers, AABB, BoundingSphere, Frustum
│   ├── w3drs-ecs/           # World, Scheduler, components — no GPU dependency
│   ├── w3drs-renderer/      # wgpu pipeline, systems, asset registry
│   ├── w3drs-assets/        # Mesh, Material, Vertex, glTF loader
│   └── w3drs-wasm/          # wasm-bindgen glue, public JS API
├── examples/
│   └── native-triangle/     # Desktop smoke test (winit)
├── xtask/                   # cargo xtask runner (www / client)
└── www/                     # Vite + TypeScript demo consuming the WASM package
```

### Naming Conventions

| Element | Convention | Example |
|---------|-----------|---------|
| **Crates** | `w3drs-<domain>` kebab-case | `w3drs-renderer`, `w3drs-ecs` |
| **Structs / traits** | `PascalCase` | `RenderState`, `AnyStorage` |
| **Components** | `PascalCase` + `Component` suffix | `TransformComponent` |
| **Systems (functions)** | `snake_case` + `_system` suffix | `frustum_culling_system` |
| **Shader files** | `snake_case.wgsl` | `pbr.wgsl`, `shadow_depth.wgsl` |
| **Public constants** | `SCREAMING_SNAKE_CASE` | `OBJECT_ALIGN`, `MAX_OBJECTS` |

---

## Code Cleanliness

### Forbidden in Production Code

- Quick-fix comments (`// TODO: fix later`, `// HACK`, `// WIP`)
- Commented-out code blocks
- `println!` / `dbg!` statements (use `log::debug!` instead)
- Unused `#[allow(dead_code)]` attributes
- Placeholder implementations without documentation

### Acceptable Markers

- `// TODO: <clear technical description>` — documented future work
- `// NOTE: <explanation>` — non-obvious invariant or constraint
- `// PERF: <note>` — performance-related observation
- `// SAFETY: <reason>` — mandatory before every `unsafe` block

### Required Practices

- Complete, compilable implementations
- `?` propagation over manual `unwrap()` in fallible paths
- Meaningful identifiers — avoid `tmp`, `foo`, `data2`
- `rustfmt`-formatted code (`cargo fmt`)
- Zero `clippy` warnings (`cargo clippy -- -D warnings`)

---

## Logging

Use the `log` crate with the `console_log` bridge in WASM:

```rust
// ❌ Don't do this
println!("uploading mesh");

// ✅ Do this instead
log::debug!("uploading mesh: {} vertices", mesh.vertices.len());
log::error!("GPU device lost: {}", e);
```

### Log Channels (by level convention)

| Level | When to use |
|-------|-------------|
| `log::error!` | Unrecoverable failures (device lost, missing adapter) |
| `log::warn!`  | Recoverable issues (missing optional attribute, fallback used) |
| `log::info!`  | Engine lifecycle events (init, resize, shutdown) |
| `log::debug!` | Asset uploads, system run traces |

**No logging inside per-frame hot paths.** The `render()` method and ECS systems run every frame — use conditional debug flags or frame counters if sampling is needed.

---

## Error Handling

Every fallible operation must propagate errors explicitly:

```rust
// ✅ Propagate with ?
pub fn upload_mesh(mesh: &Mesh, device: &wgpu::Device) -> Result<GpuMesh, EngineError> {
    let vb = device.create_buffer_init(...);
    Ok(GpuMesh { vertex_buffer: vb, ... })
}

// ❌ Don't panic in library code
let adapter = instance.request_adapter(...).await.unwrap(); // panic on real devices
```

### Error Types

- Define domain errors with `thiserror::Error` in each crate
- Prefer typed errors over `Box<dyn Error>` for public APIs
- Convert to `JsValue` only at the WASM boundary (`engine.rs`)

---

## Architecture Guidelines

### GPU Resources

- **Bind group layouts**: created once in `RenderState::new()`, reused across frames
- **Uniform buffers**: `write_buffer` each frame; never recreate per-frame
- **Mesh/material buffers**: created at upload time, never mutated; destroy on entity removal
- **Depth texture**: owned by `GpuContext`, recreated only on resize
- **Bind group limit**: WebGPU supports max 4 groups — current layout uses groups 0–2

```rust
// ✅ Correct: reuse existing buffer
self.context.queue.write_buffer(&buf, 0, bytemuck::bytes_of(&uniforms));

// ❌ Wrong: recreating buffer every frame is expensive
let buf = device.create_buffer_init(...); // allocates GPU memory every frame
```

### ECS Design

- Components are plain data structs — no methods that query the world
- Systems are free functions `fn foo_system(world: &mut World, dt: f32, t: f32)`
- Tag components (e.g. `CulledComponent`) are zero-size structs
- `World` uses `TypeId`-keyed sparse storage — each type in its own `HashMap`

### WASM API Constraints

- The public `#[wasm_bindgen]` API only accepts primitives (`f32`, `u32`, `&[u8]`)
- No Rust struct types exposed directly — use opaque handles (`u32`)
- All mutations go through methods on `W3drsEngine`
- Allocations crossing the WASM boundary use `Vec<u32>` (flat arrays)

---

## GPU / WGSL Guidelines

### Uniform Buffer Alignment

WGSL structs follow extended alignment rules — **not** the same as `repr(C)`:

| WGSL type | Alignment | Size |
|-----------|-----------|------|
| `f32`     | 4         | 4    |
| `vec2<f32>` | 8       | 8    |
| `vec3<f32>` | **16**  | 12   |
| `vec4<f32>` | 16      | 16   |
| `mat4x4<f32>` | 16   | 64   |

A struct ending with `vec3<f32>` is padded to the next 16-byte boundary — **use individual `f32` padding fields** to match the Rust `bytemuck::Pod` struct exactly.

```wgsl
// ✅ Explicit padding — Rust struct is 256 bytes, WGSL struct is 256 bytes
struct FrameUniforms {
    ...
    total_time: f32,
    _pad2a: f32, _pad2b: f32, _pad2c: f32,  // NOT vec3<f32>
}
```

### Dynamic Offsets

Per-object uniforms use a single buffer with `OBJECT_ALIGN = 256` byte stride. Never create one buffer per draw call.

---

## Code Review Checklist

### Before Submitting

**Functionality**
- [ ] `cargo check -p <crate>` passes
- [ ] `cargo clippy -- -D warnings` clean
- [ ] `cargo fmt --check` passes
- [ ] `cargo test -p w3drs-math -p w3drs-ecs -p w3drs-assets` passes

**Code Quality**
- [ ] No `unwrap()` / `expect()` in library code without documented invariant
- [ ] No commented-out code or debug statements
- [ ] WGSL struct sizes verified to match Rust counterparts (`std::mem::size_of`)
- [ ] No per-frame GPU allocations

**Documentation**
- [ ] Public items have doc comments (in English)
- [ ] Non-obvious invariants explained inline
- [ ] `unsafe` blocks have `// SAFETY:` comment

**Architecture**
- [ ] New crate dependencies justified in PR description
- [ ] No circular crate dependencies
- [ ] WASM API changes are backwards-compatible or noted as breaking

---

## Examples Policy

### Examples Must Stay in Sync with the Engine

Every engine API change that affects the public surface (`RenderState`, `GpuContext`, `AssetRegistry`, bind group layout, shader interface) **must be reflected in all examples in the same PR**:

- `examples/native-triangle/` — native desktop smoke test; uses `RenderState` and the full render loop directly
- `www/src/main.ts` — WASM/browser demo; uses the `W3drsEngine` public JS API

**Checklist for API-breaking changes:**

- [ ] `examples/native-triangle/src/main.rs` updated and compiles
- [ ] `www/src/main.ts` updated (new methods, removed methods, changed signatures)
- [ ] `cargo xtask check` passes on both native and wasm32 targets
- [ ] New features have a visible counterpart in at least one example

If an example cannot yet demonstrate a new feature (e.g. native-only ray tracing), add a `// TODO:` comment in the relevant example with a tracking reference.

---

## Git & Branches

### Pre-commit Hook

A pre-commit hook runs `cargo check` on both native and `wasm32-unknown-unknown` targets before every commit. Install it once after cloning:

```bash
cargo xtask setup-hooks
```

The hook source lives in `.githooks/pre-commit` (tracked in the repo). The hook runs:

1. `cargo check --workspace --exclude w3drs-wasm` — all native crates including examples
2. `cargo check -p w3drs-wasm --target wasm32-unknown-unknown` — WASM API

You can run the same checks manually at any time:

```bash
cargo xtask check
```

**Emergency bypass (use sparingly — only if the hook itself is broken):**

```bash
# Unix
SKIP_HOOKS=1 git commit

# Windows PowerShell
git commit --no-verify
```

Prefer fixing the underlying error instead of skipping the hook.

Run additionally before submitting a PR:

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test -p w3drs-math -p w3drs-ecs -p w3drs-assets
```

### Branch Model

- `main` — production; only tested, released code
- `develop` — default active branch for ongoing work
- `feature/<name>` — all new work; target `develop` in PRs
- Hotfixes targeting `main` must be merged back into `develop`

### Commit Message Format

```
type(scope): brief description

- Detail 1
- Detail 2
```

Types: `feat`, `fix`, `refactor`, `docs`, `perf`, `test`, `chore`

Scope examples: `renderer`, `ecs`, `assets`, `wasm`, `shader`

---

## Build Commands

```bash
# Install pre-commit hook (run once after cloning)
cargo xtask setup-hooks

# Check native + wasm32 targets (same as pre-commit hook)
cargo xtask check

# Build WASM + start Vite dev server
cargo xtask www

# Build and run native desktop example
cargo xtask client

# Run unit tests (no GPU needed)
cargo test -p w3drs-math -p w3drs-ecs -p w3drs-assets

# Lint
cargo clippy -- -D warnings
cargo fmt --check
```

---

## Versioning

Follow semver in the **0.x** pre-release phase:

- **patch** (`z`): bug fixes, docs, internal refactors
- **minor** (`y`): new features; breaking API changes are possible but must be noted in the PR
- **1.0.0**: reserved for an explicit API stability decision

Update all modified crates' `version` field in `Cargo.toml` with each PR. The workspace root `Cargo.toml` uses `version.workspace = true` for crates that share the workspace version.

---

## Markdown Files Policy

- Only `README.md` and `CONTRIBUTING.md` are allowed at the repository root
- Design notes and work-in-progress documents go in `work-in-progress/`
- Completed plans may be archived in `work-in-progress/archive/`
- Keep documentation up to date in the same PR that changes the code
