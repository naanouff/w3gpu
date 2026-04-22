//! Integration tests for the GPU Hi-Z occlusion-cull compute pass.
//!
//! Each test builds a synthetic Hi-Z texture filled with a known constant depth,
//! feeds hand-crafted EntityCullData directly into the cull pass, reads back the
//! resulting DrawIndexedIndirectArgs, and asserts on `instance_count`.
//!
//! Tests are skipped gracefully when no GPU adapter is available (CI without GPU).

use std::mem::size_of;

use bytemuck;
use w3drs_renderer::{CullPass, CullUniforms, DrawIndexedIndirectArgs, EntityCullData};

// ── GPU context ────────────────────────────────────────────────────────────────

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

/// Returns `None` if no GPU adapter is available (headless CI).
fn try_gpu() -> Option<Gpu> {
    pollster::block_on(async {
        let inst = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = inst
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::None,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .ok()?;
        Some(Gpu { device, queue })
    })
}

// ── Scene helpers ──────────────────────────────────────────────────────────────

/// Camera at (0, 0, 10) looking at origin, perspective 60° FOV, aspect=1.
///
/// Depth conventions derived from this setup:
/// - World z ≈  9.9  →  NDC z ≈ 0.00  (near plane)
/// - World z ≈  0.0  →  NDC z ≈ 0.99  (10 units from camera)
/// - World z ≈ -90.0 →  NDC z ≈ 1.00  (far plane)
fn standard_view_proj() -> [[f32; 4]; 4] {
    let view = glam::Mat4::look_at_rh(
        glam::Vec3::new(0.0, 0.0, 10.0),
        glam::Vec3::ZERO,
        glam::Vec3::Y,
    );
    let proj = glam::Mat4::perspective_rh(60_f32.to_radians(), 1.0, 0.1, 100.0);
    (proj * view).to_cols_array_2d()
}

/// Create a 64×64 R32Float Hi-Z texture filled with a constant `depth`.
///
/// One mip level. Suitable for `CullPass::rebuild_hiz_bg`.
fn make_hiz(gpu: &Gpu, depth: f32) -> (wgpu::Texture, wgpu::TextureView) {
    const W: u32 = 64;
    const H: u32 = 64;
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("test hiz"),
        size: wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let data: Vec<f32> = vec![depth; (W * H) as usize];
    gpu.queue.write_texture(
        texture.as_image_copy(),
        bytemuck::cast_slice(&data),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(W * 4), // 64 × 4 bytes = 256, aligned to COPY_BYTES_PER_ROW_ALIGNMENT
            rows_per_image: Some(H),
        },
        wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&Default::default());
    (texture, view)
}

/// Build an `EntityCullData` from a world-space AABB.
fn entity(aabb_min: [f32; 3], aabb_max: [f32; 3]) -> EntityCullData {
    EntityCullData {
        aabb_min,
        first_index: 0,
        aabb_max,
        index_count: 36,
        base_vertex: 0,
        _pad: [0; 3],
    }
}

/// Build CullUniforms for the standard 64×64 test texture (1 mip level).
fn uniforms(view_proj: [[f32; 4]; 4], entity_count: u32, cull_enabled: u32) -> CullUniforms {
    CullUniforms {
        view_proj,
        screen_size: [64.0, 64.0],
        entity_count,
        mip_levels: 1,
        cull_enabled,
        _pad: [0; 3],
    }
}

// ── Cull-pass runner ───────────────────────────────────────────────────────────

/// Upload entities + uniforms, dispatch cull compute, read back `instance_count`
/// for each entity. Returns a `Vec<u32>` parallel to `entities`.
fn run_cull(
    gpu: &Gpu,
    cull: &CullPass,
    readback: &wgpu::Buffer,
    uni: CullUniforms,
    entities: &[EntityCullData],
) -> Vec<u32> {
    let n = uni.entity_count as usize;
    assert!(n > 0 && n <= entities.len());

    gpu.queue
        .write_buffer(&cull.cull_uniform_buf, 0, bytemuck::bytes_of(&uni));
    gpu.queue
        .write_buffer(&cull.entity_cull_buf, 0, bytemuck::cast_slice(entities));

    let stride = size_of::<DrawIndexedIndirectArgs>() as u64;
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    cull.encode(&mut enc, uni.entity_count);
    enc.copy_buffer_to_buffer(&cull.entity_indirect_buf, 0, readback, 0, n as u64 * stride);
    gpu.queue.submit([enc.finish()]);

    let slice = readback.slice(..n as u64 * stride);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    gpu.device.poll(wgpu::Maintain::Wait);
    let view = slice.get_mapped_range();
    let args: &[DrawIndexedIndirectArgs] = bytemuck::cast_slice(&view);
    let result = args[..n].iter().map(|a| a.instance_count).collect();
    drop(view);
    readback.unmap();
    result
}

/// Create a MAP_READ | COPY_DST readback buffer sized for `n` indirect args.
fn make_readback(gpu: &Gpu, n: usize) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (n * size_of::<DrawIndexedIndirectArgs>()) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    })
}

// ── Tests ──────────────────────────────────────────────────────────────────────

/// When cull_enabled = 0 every entity must have instance_count = 1,
/// regardless of its AABB or the depth in the Hi-Z texture.
#[test]
fn cull_disabled_marks_all_visible() {
    let Some(gpu) = try_gpu() else { return };
    let mut cull = CullPass::new(&gpu.device);
    let (_tex, view) = make_hiz(&gpu, 0.0); // extreme occluder — irrelevant when disabled
    cull.rebuild_hiz_bg(&gpu.device, &view);
    let readback = make_readback(&gpu, 3);

    let entities = [
        entity([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5]), // at origin
        entity([5.0, 5.0, -10.0], [6.0, 6.0, -9.0]), // far right
        entity([-0.5, -0.5, 8.0], [0.5, 0.5, 9.0]),  // near camera
    ];
    let result = run_cull(
        &gpu,
        &cull,
        &readback,
        uniforms(standard_view_proj(), 3, 0 /* DISABLED */),
        &entities,
    );

    assert_eq!(result, [1, 1, 1], "cull_enabled=0 must pass every entity");
}

/// An entity in the frustum with a clear Hi-Z (depth=1.0, no occluder)
/// must never be culled — its depth_near is always < 1.0.
#[test]
fn visible_entity_clear_sky() {
    let Some(gpu) = try_gpu() else { return };
    let mut cull = CullPass::new(&gpu.device);
    let (_tex, view) = make_hiz(&gpu, 1.0); // Hi-Z = far plane (nothing occluding)
    cull.rebuild_hiz_bg(&gpu.device, &view);
    let readback = make_readback(&gpu, 1);

    // Entity at origin, 10 units from camera → NDC z ≈ 0.99 < 1.0 → visible
    let result = run_cull(
        &gpu,
        &cull,
        &readback,
        uniforms(standard_view_proj(), 1, 1),
        &[entity([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])],
    );

    assert_eq!(result, [1], "entity in clear scene must be visible");
}

/// An entity far from the camera, behind a very close occluder (Hi-Z ≈ 0.0),
/// must be culled: depth_near >> 0.0.
#[test]
fn entity_behind_close_occluder() {
    let Some(gpu) = try_gpu() else { return };
    let mut cull = CullPass::new(&gpu.device);
    // Hi-Z = 0.1: occluder is at ~10% of the depth range (≈1 unit from near plane).
    // Entity at origin has NDC z ≈ 0.99 > 0.1 → should be culled.
    let (_tex, view) = make_hiz(&gpu, 0.1);
    cull.rebuild_hiz_bg(&gpu.device, &view);
    let readback = make_readback(&gpu, 1);

    let result = run_cull(
        &gpu,
        &cull,
        &readback,
        uniforms(standard_view_proj(), 1, 1),
        &[entity([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])],
    );

    assert_eq!(result, [0], "entity behind close occluder must be culled");
}

/// An entity whose AABB straddles the camera near-plane (some corners behind
/// the camera, some in front) must NEVER be culled, regardless of Hi-Z depth.
/// This is the "any_behind" conservative-cull guard.
#[test]
fn near_plane_straddler_never_culled() {
    let Some(gpu) = try_gpu() else { return };
    let mut cull = CullPass::new(&gpu.device);
    // Worst-case Hi-Z: depth = 0.0 (occluder right at the near plane).
    let (_tex, view) = make_hiz(&gpu, 0.0);
    cull.rebuild_hiz_bg(&gpu.device, &view);
    let readback = make_readback(&gpu, 1);

    // AABB from z=9 to z=11 with camera at z=10:
    // corners at z=9  → view_z=-1 → clip.w=1  (in front)
    // corners at z=11 → view_z=+1 → clip.w=-1 (behind)  → any_behind=true → depth_near=0.0
    let result = run_cull(
        &gpu,
        &cull,
        &readback,
        uniforms(standard_view_proj(), 1, 1),
        &[entity([-0.5, -0.5, 9.0], [0.5, 0.5, 11.0])],
    );

    assert_eq!(result, [1], "near-plane straddler must never be culled");
}

/// An entity entirely behind the camera (all clip.w ≤ 0) must be culled.
#[test]
fn all_corners_behind_camera() {
    let Some(gpu) = try_gpu() else { return };
    let mut cull = CullPass::new(&gpu.device);
    let (_tex, view) = make_hiz(&gpu, 1.0);
    cull.rebuild_hiz_bg(&gpu.device, &view);
    let readback = make_readback(&gpu, 1);

    // Entity at z=11 to 13 — entirely behind camera at z=10.
    // All corners have view_z > 0 → clip.w < 0 → all_behind=true.
    let result = run_cull(
        &gpu,
        &cull,
        &readback,
        uniforms(standard_view_proj(), 1, 1),
        &[entity([-0.5, -0.5, 11.0], [0.5, 0.5, 13.0])],
    );

    assert_eq!(result, [0], "entity fully behind camera must be culled");
}

/// An entity whose projected AABB lies entirely outside the XY frustum must
/// be culled (coarse frustum reject before the Hi-Z depth test).
#[test]
fn entity_outside_frustum_xy() {
    let Some(gpu) = try_gpu() else { return };
    let mut cull = CullPass::new(&gpu.device);
    let (_tex, view) = make_hiz(&gpu, 1.0); // clear sky — depth test would pass
    cull.rebuild_hiz_bg(&gpu.device, &view);
    let readback = make_readback(&gpu, 1);

    // Camera fov=60°, aspect=1, at z=10 looking at origin.
    // Frustum half-width at z=0 (view_z=-10) = 10 * tan(30°) ≈ 5.77 world units.
    // Entity at x=10..11 → NDC x = proj[0][0]*10/10 ≈ 1.73 → entirely right of frustum.
    let result = run_cull(
        &gpu,
        &cull,
        &readback,
        uniforms(standard_view_proj(), 1, 1),
        &[entity([10.0, -0.5, -0.5], [11.0, 0.5, 0.5])],
    );

    assert_eq!(result, [0], "entity outside XY frustum must be culled");
}

/// Mixed scene: verify per-entity independence.
/// Entity 0 — in frustum, Hi-Z clear → visible (1).
/// Entity 1 — outside XY frustum → culled (0).
/// Entity 2 — in frustum, Hi-Z clear → visible (1).
#[test]
fn multiple_entities_mixed() {
    let Some(gpu) = try_gpu() else { return };
    let mut cull = CullPass::new(&gpu.device);
    let (_tex, view) = make_hiz(&gpu, 1.0);
    cull.rebuild_hiz_bg(&gpu.device, &view);
    let readback = make_readback(&gpu, 3);

    let entities = [
        entity([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5]),  // visible
        entity([10.0, -0.5, -0.5], [11.0, 0.5, 0.5]), // outside frustum → culled
        entity([-1.0, -1.0, 7.0], [1.0, 1.0, 9.0]),   // near camera → visible
    ];
    let result = run_cull(
        &gpu,
        &cull,
        &readback,
        uniforms(standard_view_proj(), 3, 1),
        &entities,
    );

    assert_eq!(result, [1, 0, 1], "per-entity culling must be independent");
}

/// Monotonicity: enabling culling can only reduce draw count, never increase it.
///
/// Runs the same scene with cull ON and OFF; verifies:
///   hiz_visible_count ≤ total_count
///   all_visible_count  = total_count
#[test]
fn monotonicity_cull_on_never_exceeds_cull_off() {
    let Some(gpu) = try_gpu() else { return };
    let mut cull = CullPass::new(&gpu.device);
    // Hi-Z = 0.5: some entities will be culled, some won't.
    let (_tex, view) = make_hiz(&gpu, 0.5);
    cull.rebuild_hiz_bg(&gpu.device, &view);

    // Build a varied scene: different depths, one outside frustum.
    let entities = [
        entity([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5]), // far   → likely culled
        entity([-0.5, -0.5, 8.0], [0.5, 0.5, 9.0]),  // near  → might pass
        entity([10.0, -0.5, -0.5], [11.0, 0.5, 0.5]), // off-screen → culled
        entity([-0.5, -0.5, 9.0], [0.5, 0.5, 11.0]), // straddler → never culled
    ];
    let n = entities.len() as u32;
    let readback = make_readback(&gpu, entities.len());

    let off = run_cull(
        &gpu,
        &cull,
        &readback,
        uniforms(standard_view_proj(), n, 0 /* OFF */),
        &entities,
    );
    let on = run_cull(
        &gpu,
        &cull,
        &readback,
        uniforms(standard_view_proj(), n, 1 /* ON */),
        &entities,
    );

    let off_sum: u32 = off.iter().sum();
    let on_sum: u32 = on.iter().sum();

    // With cull OFF every in-cull-buf entity gets instance_count=1
    // (XY frustum and behind-camera are still applied even when cull_enabled=0
    //  is NOT the case — cull_enabled=0 bypasses ALL culling).
    assert_eq!(off_sum, n, "cull OFF must pass all entities");
    assert!(
        on_sum <= off_sum,
        "cull ON ({on_sum}) must not exceed cull OFF ({off_sum})"
    );
}

/// The near-plane straddler must remain visible even with Hi-Z = 0.0.
/// Distinct from `near_plane_straddler_never_culled` — this verifies
/// the invariant holds alongside other entities being culled.
#[test]
fn straddler_visible_among_culled_neighbours() {
    let Some(gpu) = try_gpu() else { return };
    let mut cull = CullPass::new(&gpu.device);
    let (_tex, view) = make_hiz(&gpu, 0.0); // everything gets culled unless straddling
    cull.rebuild_hiz_bg(&gpu.device, &view);
    let readback = make_readback(&gpu, 3);

    let entities = [
        entity([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5]), // far → culled (depth > 0)
        entity([-0.5, -0.5, 9.0], [0.5, 0.5, 11.0]), // straddler → depth_near=0 → visible
        entity([-0.5, -0.5, 7.0], [0.5, 0.5, 9.0]),  // near but in front → depth≈0.9 > 0 → culled
    ];
    let result = run_cull(
        &gpu,
        &cull,
        &readback,
        uniforms(standard_view_proj(), 3, 1),
        &entities,
    );

    assert_eq!(result[1], 1, "straddler must stay visible when Hi-Z = 0.0");
    assert_eq!(result[0], 0, "far entity must be culled with Hi-Z = 0.0");
}
