// GPU Hi-Z occlusion culling.
// group(0): CullUniforms + EntityData[] + DrawArgs[] (read_write)
// group(1): Hi-Z texture (all mips)
//
// For each entity: project world-space AABB to screen, pick the appropriate
// Hi-Z mip, compare nearest depth against the stored max-depth → write
// instance_count = 0 (culled) or 1 (visible) into DrawIndexedIndirectArgs.

struct CullUniforms {
    view_proj:    mat4x4<f32>,   // offset   0, size 64
    screen_size:  vec2<f32>,     // offset  64, size  8
    entity_count: u32,           // offset  72, size  4
    mip_levels:   u32,           // offset  76, size  4
    cull_enabled: u32,           // offset  80, size  4
    // 12 bytes implicit padding → struct size = 96, matches Rust CullUniforms
}

struct EntityData {
    aabb_min:    vec3<f32>,
    first_index: u32,
    aabb_max:    vec3<f32>,
    index_count: u32,
    base_vertex: i32,
    _pad0: u32, _pad1: u32, _pad2: u32,
}

struct DrawArgs {
    index_count:    u32,
    instance_count: u32,
    first_index:    u32,
    base_vertex:    i32,
    first_instance: u32,
}

@group(0) @binding(0) var<uniform>            cull:      CullUniforms;
@group(0) @binding(1) var<storage, read>       entities:  array<EntityData>;
@group(0) @binding(2) var<storage, read_write> draw_args: array<DrawArgs>;
// stats[0] = frustum_rejected, stats[1] = hiz_rejected.
// CPU clears to [0, 0] before each cull dispatch.
@group(0) @binding(3) var<storage, read_write> stats:     array<atomic<u32>, 2>;
@group(1) @binding(0) var hiz_tex: texture_2d<f32>;

@compute @workgroup_size(64)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let eid = id.x;
    if eid >= cull.entity_count { return; }
    let e = entities[eid];

    // Write fixed fields unconditionally
    draw_args[eid].index_count    = e.index_count;
    draw_args[eid].first_index    = e.first_index;
    draw_args[eid].base_vertex    = e.base_vertex;
    draw_args[eid].first_instance = eid;

    if cull.cull_enabled == 0u {
        draw_args[eid].instance_count = 1u;
        return;
    }

    // Project 8 AABB corners into NDC
    let corners = array<vec3<f32>, 8>(
        vec3<f32>(e.aabb_min.x, e.aabb_min.y, e.aabb_min.z),
        vec3<f32>(e.aabb_max.x, e.aabb_min.y, e.aabb_min.z),
        vec3<f32>(e.aabb_min.x, e.aabb_max.y, e.aabb_min.z),
        vec3<f32>(e.aabb_max.x, e.aabb_max.y, e.aabb_min.z),
        vec3<f32>(e.aabb_min.x, e.aabb_min.y, e.aabb_max.z),
        vec3<f32>(e.aabb_max.x, e.aabb_min.y, e.aabb_max.z),
        vec3<f32>(e.aabb_min.x, e.aabb_max.y, e.aabb_max.z),
        vec3<f32>(e.aabb_max.x, e.aabb_max.y, e.aabb_max.z),
    );

    var ndc_min    = vec3<f32>( 1e9,  1e9,  1e9);
    var ndc_max    = vec3<f32>(-1e9, -1e9, -1e9);
    var all_behind  = true;
    var any_behind  = false;

    for (var i = 0; i < 8; i++) {
        let clip = cull.view_proj * vec4<f32>(corners[i], 1.0);
        if clip.w > 0.0 {
            all_behind = false;
            let ndc = clip.xyz / clip.w;
            ndc_min = min(ndc_min, ndc);
            ndc_max = max(ndc_max, ndc);
        } else {
            any_behind = true;
        }
    }

    if all_behind {
        draw_args[eid].instance_count = 0u;
        atomicAdd(&stats[0], 1u);
        return;
    }

    // Coarse 6-plane frustum cull (skip XY/far when any corner is behind the
    // near plane — conservative for near-plane straddlers). NDC z range is
    // [0, 1] (wgpu convention), so `ndc_min.z > 1.0` means the closest point of
    // the AABB is already past the far plane → fully outside frustum.
    if !any_behind && (ndc_max.x < -1.0 || ndc_min.x > 1.0 ||
                       ndc_max.y < -1.0 || ndc_min.y > 1.0 ||
                       ndc_min.z > 1.0) {
        draw_args[eid].instance_count = 0u;
        atomicAdd(&stats[0], 1u);
        return;
    }

    // Clamp to view frustum for UV derivation
    let ndc_lo = max(ndc_min, vec3<f32>(-1.0, -1.0,  0.0));
    let ndc_hi = min(ndc_max, vec3<f32>( 1.0,  1.0,  1.0));

    // NDC → UV  (Y-flip: NDC Y-up, UV Y-down)
    let uv_a  = ndc_lo.xy * vec2<f32>( 0.5, -0.5) + vec2<f32>(0.5);
    let uv_b  = ndc_hi.xy * vec2<f32>( 0.5, -0.5) + vec2<f32>(0.5);
    let uv_lo = min(uv_a, uv_b);
    let uv_hi = max(uv_a, uv_b);

    // If any corner is behind the near plane the object straddles it → never cull.
    // Otherwise use the minimum NDC z of the visible corners (closest to camera).
    let depth_near = select(clamp(ndc_min.z, 0.0, 1.0), 0.0, any_behind);

    // Pick mip level M so 1 texel at M covers ≈ box-pixel-size. With 4 corner
    // samples the projected box is guaranteed to fit inside the 2×2 footprint
    // (a box of N pixels straddles at most 2 texels per axis at mip M).
    let size_px = (uv_hi - uv_lo) * cull.screen_size;
    let max_dim = max(max(size_px.x, size_px.y), 1.0);
    let mip_f   = log2(max_dim);
    let mip_i   = clamp(i32(ceil(mip_f)), 0, i32(cull.mip_levels) - 1);
    let mip_u   = u32(mip_i);

    // Sample 4 corners of the projected footprint at the chosen mip and take
    // the MAX (Hi-Z stores the farthest visible depth per texel, so the MAX
    // across the 4 corners is the farthest visible surface anywhere in the
    // box's screen-space coverage).
    let hiz_size  = vec2<f32>(textureDimensions(hiz_tex, mip_u));
    let texel_lo  = clamp(vec2<i32>(uv_lo * hiz_size),
                          vec2<i32>(0),
                          vec2<i32>(hiz_size) - vec2<i32>(1));
    let texel_hi  = clamp(vec2<i32>(uv_hi * hiz_size),
                          vec2<i32>(0),
                          vec2<i32>(hiz_size) - vec2<i32>(1));
    let d00 = textureLoad(hiz_tex, vec2<i32>(texel_lo.x, texel_lo.y), mip_i).r;
    let d10 = textureLoad(hiz_tex, vec2<i32>(texel_hi.x, texel_lo.y), mip_i).r;
    let d01 = textureLoad(hiz_tex, vec2<i32>(texel_lo.x, texel_hi.y), mip_i).r;
    let d11 = textureLoad(hiz_tex, vec2<i32>(texel_hi.x, texel_hi.y), mip_i).r;
    let hiz_depth = max(max(d00, d10), max(d01, d11));

    // Occluded when nearest point of AABB is farther than Hi-Z stored max-depth
    if depth_near > hiz_depth {
        draw_args[eid].instance_count = 0u;
        atomicAdd(&stats[1], 1u);
    } else {
        draw_args[eid].instance_count = 1u;
    }
}
