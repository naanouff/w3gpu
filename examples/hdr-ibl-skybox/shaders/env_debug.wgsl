// Fullscreen debug: HDR équirect (vue caméra) ou faces cubemap IBL (préfiltre / irradiance).
// group(0): matrices + view_mode + prefilter_lod
// group(1): HDR 2D, sampler, prefiltered cube, irradiance cube

struct SkyUniforms {
    inv_view_projection: mat4x4<f32>,
    camera_position:     vec3<f32>,
    _pad0:               f32,
    /// 0 = HDR sky ; 1–6 = préfiltre mip `prefilter_lod` face +X..-Z ; 7–12 = irradiance face.
    view_mode:           u32,
    _pad1:               u32,
    prefilter_lod:       f32,
    _pad2:               f32,
}

@group(0) @binding(0) var<uniform> sky: SkyUniforms;
@group(1) @binding(0) var hdr_tex: texture_2d<f32>;
@group(1) @binding(1) var env_samp: sampler;
@group(1) @binding(2) var prefiltered_map: texture_cube<f32>;
@group(1) @binding(3) var irradiance_map: texture_cube<f32>;

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) ndc_xy: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var p = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    let v = p[vi];
    var o: VsOut;
    o.clip_pos = vec4<f32>(v, 1.0, 1.0);
    o.ndc_xy = v;
    return o;
}

fn direction_from_clip(ndc_xy: vec2<f32>) -> vec3<f32> {
    let ndc = vec4<f32>(ndc_xy, 1.0, 1.0);
    var world_h = sky.inv_view_projection * ndc;
    world_h = world_h / world_h.w;
    return normalize(world_h.xyz - sky.camera_position);
}

fn sample_equirect(dir: vec3<f32>) -> vec3<f32> {
    let n = normalize(dir);
    let u = 0.5 + atan2(n.z, n.x) / (2.0 * 3.141592653589793);
    let v = 0.5 - asin(clamp(n.y, -1.0, 1.0)) / 3.141592653589793;
    let dims = vec2<f32>(textureDimensions(hdr_tex));
    let max_uv = (dims - vec2<f32>(1.0)) / dims;
    let uv = clamp(vec2<f32>(u, v), vec2<f32>(0.0), max_uv);
    return textureSample(hdr_tex, env_samp, uv).rgb;
}

/// Aligné sur `face_direction` dans `ibl.rs` (glTF cubemap faces).
fn face_uv_to_dir(face: u32, u: f32, v: f32) -> vec3<f32> {
    let t_u = u * 2.0 - 1.0;
    let t_v = v * 2.0 - 1.0;
    switch face {
        case 0u: { return normalize(vec3<f32>(1.0, -t_v, -t_u)); }
        case 1u: { return normalize(vec3<f32>(-1.0, -t_v, t_u)); }
        case 2u: { return normalize(vec3<f32>(t_u, 1.0, t_v)); }
        case 3u: { return normalize(vec3<f32>(t_u, -1.0, -t_v)); }
        case 4u: { return normalize(vec3<f32>(t_u, -t_v, 1.0)); }
        case 5u: { return normalize(vec3<f32>(-t_u, -t_v, -1.0)); }
        default: { return vec3<f32>(0.0, 1.0, 0.0); }
    }
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // UV écran 0–1 (Y vers le haut NDC → v augmente vers le haut de l’écran)
    let uv = in.ndc_xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    let mode = sky.view_mode;

    if mode == 0u {
        let dir = direction_from_clip(in.ndc_xy);
        return vec4<f32>(sample_equirect(dir), 1.0);
    }

    if mode >= 1u && mode <= 6u {
        let face = mode - 1u;
        let dir = face_uv_to_dir(face, uv.x, uv.y);
        let max_l = max(f32(textureNumLevels(prefiltered_map)) - 1.0, 0.0);
        let lod = clamp(sky.prefilter_lod, 0.0, max_l);
        let rgb = textureSampleLevel(prefiltered_map, env_samp, dir, lod).rgb;
        return vec4<f32>(rgb, 1.0);
    }

    if mode >= 7u && mode <= 12u {
        let face = mode - 7u;
        let dir = face_uv_to_dir(face, uv.x, uv.y);
        let rgb = textureSample(irradiance_map, env_samp, dir).rgb;
        return vec4<f32>(rgb, 1.0);
    }

    return vec4<f32>(1.0, 0.0, 1.0, 1.0);
}
