// GPU Hi-Z occlusion culling.
// group(0): CullUniforms + EntityData[] + DrawArgs[] (read_write)
// group(1): Hi-Z texture (all mips)
//
// For each entity: project world-space AABB to screen, pick the appropriate
// Hi-Z mip, compare nearest depth against the stored max-depth → write
// instance_count = 0 (culled) or 1 (visible) into DrawIndexedIndirectArgs.

struct CullUniforms {
    view_proj:    mat4x4<f32>,
    screen_size:  vec2<f32>,
    entity_count: u32,
    mip_levels:   u32,
    cull_enabled: u32,
    _pad:         vec3<u32>,
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

    var ndc_min  = vec3<f32>( 1e9,  1e9,  1e9);
    var ndc_max  = vec3<f32>(-1e9, -1e9, -1e9);
    var all_behind = true;

    for (var i = 0; i < 8; i++) {
        let clip = cull.view_proj * vec4<f32>(corners[i], 1.0);
        if clip.w > 0.0 { all_behind = false; }
        let w   = max(clip.w, 0.0001);
        let ndc = clip.xyz / w;
        ndc_min = min(ndc_min, ndc);
        ndc_max = max(ndc_max, ndc);
    }

    if all_behind {
        draw_args[eid].instance_count = 0u;
        return;
    }

    // Coarse XY frustum cull
    if ndc_max.x < -1.0 || ndc_min.x > 1.0 ||
       ndc_max.y < -1.0 || ndc_min.y > 1.0 {
        draw_args[eid].instance_count = 0u;
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

    // Nearest depth of this AABB (minimum NDC z = closest to camera)
    let depth_near = clamp(ndc_min.z, 0.0, 1.0);

    // Pick mip level so the 2×2 texel footprint covers the projected footprint
    let size_px = (uv_hi - uv_lo) * cull.screen_size;
    let mip_f   = log2(max(max(size_px.x, size_px.y), 1.0));
    let mip_i   = clamp(i32(ceil(mip_f)), 0, i32(cull.mip_levels) - 1);
    let mip_u   = u32(mip_i);

    // Sample Hi-Z at centre of projected footprint
    let uv_c      = (uv_lo + uv_hi) * 0.5;
    let hiz_size  = vec2<f32>(textureDimensions(hiz_tex, mip_u));
    let texel     = clamp(vec2<i32>(uv_c * hiz_size),
                          vec2<i32>(0),
                          vec2<i32>(hiz_size) - vec2<i32>(1));
    let hiz_depth = textureLoad(hiz_tex, texel, mip_i).r;

    // Occluded when nearest point of AABB is farther than Hi-Z stored max-depth
    if depth_near > hiz_depth {
        draw_args[eid].instance_count = 0u;
    } else {
        draw_args[eid].instance_count = 1u;
    }
}
