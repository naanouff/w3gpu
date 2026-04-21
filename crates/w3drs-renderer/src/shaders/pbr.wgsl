// group(0) = FrameUniforms              (per-frame, includes shadow light_view_proj)
// group(1) = instances storage buffer  (array<mat4x4<f32>>, indexed by instance_index)
// group(2) = MaterialUniforms + textures (per-material)
// group(3) = IBL (bindings 0-3) + shadow map/sampler (bindings 4-5)

struct FrameUniforms {
    projection:          mat4x4<f32>,
    view:                mat4x4<f32>,
    inv_view_projection: mat4x4<f32>,
    camera_position:     vec3<f32>,
    _pad0:               f32,
    light_direction:     vec3<f32>,
    _pad1:               f32,
    light_color:         vec3<f32>,
    ambient_intensity:   f32,
    total_time:          f32,
    _pad2a:              f32,
    _pad2b:              f32,
    _pad2c:              f32,
    // shadow data (folded in to stay within max_bind_groups = 4)
    light_view_proj:     mat4x4<f32>,
    shadow_bias:         f32,
    _pad3a: f32, _pad3b: f32, _pad3c: f32,
}

/// `KHR_texture_transform` + choix de `texCoord` (aligné `UvTransformGpu` Rust).
struct UvTransform {
    offset:    vec2<f32>,
    rotation:  f32,
    _pad0:     f32,
    scale:     vec2<f32>,
    tex_coord: u32,
    _pad1:     u32,
}

struct MaterialUniforms {
    albedo:    vec4<f32>,
    emissive:  vec4<f32>,
    metallic:  f32,
    roughness: f32,
    anisotropy_strength: f32,
    anisotropy_rotation: f32,
    ior:       f32,
    clearcoat_factor:     f32,
    clearcoat_roughness:  f32,
    _pad_main: f32,
    uv_transforms: array<UvTransform, 7>,
}

@group(0) @binding(0) var<uniform>        frame:     FrameUniforms;
@group(1) @binding(0) var<storage, read>  instances: array<mat4x4<f32>>;
@group(2) @binding(0) var<uniform>        material:  MaterialUniforms;
@group(2) @binding(1) var albedo_tex:    texture_2d<f32>;
@group(2) @binding(2) var normal_tex:    texture_2d<f32>;
@group(2) @binding(3) var mr_tex:        texture_2d<f32>; // G=roughness, B=metallic
@group(2) @binding(4) var emissive_tex:  texture_2d<f32>;
@group(2) @binding(5) var aniso_tex:            texture_2d<f32>;
@group(2) @binding(6) var clearcoat_tex:        texture_2d<f32>;
@group(2) @binding(7) var clearcoat_rough_tex:  texture_2d<f32>;
@group(2) @binding(8) var mat_sampler:          sampler;

@group(3) @binding(0) var irradiance_map:  texture_cube<f32>;
@group(3) @binding(1) var prefiltered_map: texture_cube<f32>;
@group(3) @binding(2) var brdf_lut:        texture_2d<f32>;
@group(3) @binding(3) var ibl_sampler:     sampler;
@group(3) @binding(4) var shadow_map:      texture_depth_2d;
@group(3) @binding(5) var shadow_sampler:  sampler_comparison;

struct VertexInput {
    @builtin(instance_index) instance_idx: u32,
    @location(0) position:  vec3<f32>,
    @location(1) uv0:       vec2<f32>,
    @location(2) uv1:       vec2<f32>,
    @location(3) normal:    vec3<f32>,
    @location(4) tangent:   vec3<f32>,
    @location(5) bitangent: vec3<f32>,
    @location(6) color:     vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_pos:        vec4<f32>,
    @location(0)       world_pos:       vec3<f32>,
    @location(1)       world_normal:    vec3<f32>,
    @location(2)       world_tangent:   vec3<f32>,
    @location(3)       world_bitangent: vec3<f32>,
    @location(4)       uv0:             vec2<f32>,
    @location(5)       uv1:             vec2<f32>,
    @location(6)       color:           vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    let world     = instances[in.instance_idx];
    let world_pos = world * vec4<f32>(in.position, 1.0);
    let normal_mat = mat3x3<f32>(
        world[0].xyz,
        world[1].xyz,
        world[2].xyz,
    );
    var out: VertexOutput;
    out.clip_pos        = frame.projection * frame.view * world_pos;
    out.world_pos       = world_pos.xyz;
    out.world_normal    = normalize(normal_mat * in.normal);
    out.world_tangent   = normalize(normal_mat * in.tangent);
    out.world_bitangent = normalize(normal_mat * in.bitangent);
    out.uv0             = in.uv0;
    out.uv1             = in.uv1;
    out.color           = in.color;
    return out;
}

// ── PBR helpers ───────────────────────────────────────────────────────────────

const PI: f32 = 3.14159265358979;

fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    let a  = roughness * roughness;
    let a2 = a * a;
    let nh = max(dot(n, h), 0.0);
    let d  = nh * nh * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d);
}

fn geometry_schlick_ggx(ndotv: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return ndotv / (ndotv * (1.0 - k) + k);
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let nv = max(dot(n, v), 0.0);
    let nl = max(dot(n, l), 0.0);
    return geometry_schlick_ggx(nv, roughness) * geometry_schlick_ggx(nl, roughness);
}

// KHR_materials_anisotropy — non-normative reference from Khronos sample GLSL (Burley GGX).
fn d_ggx_aniso(ndot_h: f32, tdot_h: f32, bdot_h: f32, at: f32, ab: f32) -> f32 {
    let a2 = at * ab;
    let f = vec3<f32>(ab * tdot_h, at * bdot_h, a2 * ndot_h);
    let w2 = a2 / dot(f, f);
    return a2 * w2 * w2 / PI;
}

fn v_ggx_aniso(
    ndot_l: f32, ndot_v: f32,
    bdot_v: f32, tdot_v: f32,
    tdot_l: f32, bdot_l: f32,
    at: f32, ab: f32,
) -> f32 {
    let ggxv = ndot_l * length(vec3<f32>(at * tdot_v, ab * bdot_v, ndot_v));
    let ggxl = ndot_v * length(vec3<f32>(at * tdot_l, ab * bdot_l, ndot_l));
    let denomv = ggxv + ggxl;
    return clamp(0.5 / denomv, 0.0, 1.0);
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

/// Schlick F0 for a dielectric from index of refraction (`KHR_materials_ior`).
fn dielectric_f0_from_ior(ior: f32) -> vec3<f32> {
    let t = clamp(ior, 1.0001, 256.0);
    let x = (t - 1.0) / (t + 1.0);
    let f = x * x;
    return vec3<f32>(f, f, f);
}

fn fresnel_schlick_roughness(cos_theta: f32, f0: vec3<f32>, roughness: f32) -> vec3<f32> {
    let inv = vec3<f32>(1.0 - roughness);
    return f0 + (max(inv, f0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

/// `KHR_texture_transform` — même ordre que l’exemple GLSL Khronos : `translation * rotation * scale`.
fn khr_texture_transform_uv(uv0: vec2<f32>, uv1: vec2<f32>, xf: UvTransform) -> vec2<f32> {
    let uv = select(uv0, uv1, xf.tex_coord != 0u);
    let sx = uv.x * xf.scale.x;
    let sy = uv.y * xf.scale.y;
    let c = cos(xf.rotation);
    let s = sin(xf.rotation);
    let rx = c * sx + s * sy;
    let ry = -s * sx + c * sy;
    return vec2<f32>(rx, ry) + xf.offset;
}

// 3×3 PCF shadow factor: 1.0 = fully lit, 0.0 = fully in shadow.
// textureSampleCompare must be in uniform control flow, so we always run the
// loop on clamped coordinates and use select() instead of an early return.
fn pcf_shadow(world_pos: vec3<f32>) -> f32 {
    let light_clip  = frame.light_view_proj * vec4<f32>(world_pos, 1.0);
    let ndc         = light_clip.xyz / light_clip.w;
    // NDC [-1,1] → UV [0,1], flip Y (WebGPU NDC Y-up, UV Y-down)
    let uv          = ndc.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
    let depth_ref   = ndc.z - frame.shadow_bias;
    let in_frustum  = uv.x >= 0.0 && uv.x <= 1.0 && uv.y >= 0.0 && uv.y <= 1.0 && depth_ref <= 1.0;
    // Clamp so out-of-frustum taps don't sample outside [0,1]
    let safe_uv     = clamp(uv, vec2<f32>(0.001), vec2<f32>(0.999));
    let safe_depth  = clamp(depth_ref, 0.0, 1.0);
    var shadow = 0.0;
    let texel = 1.0 / 2048.0;
    for (var xi: i32 = -1; xi <= 1; xi = xi + 1) {
        for (var yi: i32 = -1; yi <= 1; yi = yi + 1) {
            let off = vec2<f32>(f32(xi), f32(yi)) * texel;
            shadow += textureSampleCompare(shadow_map, shadow_sampler, safe_uv + off, safe_depth);
        }
    }
    return select(1.0, shadow / 9.0, in_frustum);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // ── texture sampling (`KHR_texture_transform` par slot) ───────────────────
    let uv_alb = khr_texture_transform_uv(in.uv0, in.uv1, material.uv_transforms[0u]);
    let uv_n   = khr_texture_transform_uv(in.uv0, in.uv1, material.uv_transforms[1u]);
    let uv_mr  = khr_texture_transform_uv(in.uv0, in.uv1, material.uv_transforms[2u]);
    let uv_em  = khr_texture_transform_uv(in.uv0, in.uv1, material.uv_transforms[3u]);
    let albedo_sample = textureSample(albedo_tex, mat_sampler, uv_alb);
    let mr_sample     = textureSample(mr_tex, mat_sampler, uv_mr);
    let emit_sample   = textureSample(emissive_tex, mat_sampler, uv_em);

    let albedo    = material.albedo.rgb * albedo_sample.rgb * in.color.rgb;
    let metallic  = material.metallic  * mr_sample.b;
    let roughness = clamp(material.roughness * mr_sample.g, 0.04, 1.0);
    let emissive  = material.emissive.rgb * emit_sample.rgb;

    // ── normal mapping (TBN) ──────────────────────────────────────────────────
    let normal_sample = textureSample(normal_tex, mat_sampler, uv_n).xyz;
    let n_tangent = normalize(normal_sample * 2.0 - vec3<f32>(1.0));
    let tbn = mat3x3<f32>(
        normalize(in.world_tangent),
        normalize(in.world_bitangent),
        normalize(in.world_normal),
    );
    let n = normalize(tbn * n_tangent);

    // KHR_materials_clearcoat — facteurs × textures (R / G) + `KHR_texture_transform`.
    let uv_cc = khr_texture_transform_uv(in.uv0, in.uv1, material.uv_transforms[5u]);
    let uv_cr = khr_texture_transform_uv(in.uv0, in.uv1, material.uv_transforms[6u]);
    let cc_tex_r   = textureSample(clearcoat_tex, mat_sampler, uv_cc).r;
    let cc_rough_g = textureSample(clearcoat_rough_tex, mat_sampler, uv_cr).g;
    let cc_f = clamp(material.clearcoat_factor * cc_tex_r, 0.0, 1.0);
    let cc_rough_eff = max(material.clearcoat_roughness * cc_rough_g, 0.089);
    let f0_coat = dielectric_f0_from_ior(1.5);

    // ── Cook-Torrance BRDF (direct light) ─────────────────────────────────────
    let v  = normalize(frame.camera_position - in.world_pos);
    let l  = normalize(-frame.light_direction);
    let h  = normalize(v + l);
    let f0 = mix(dielectric_f0_from_ior(material.ior), albedo, metallic);

    let nl = max(dot(n, l), 0.0);
    let nv = max(dot(n, v), 0.0);
    let nh = max(dot(n, h), 0.0);
    let denom = 4.0 * nv * nl + 0.0001;
    let f = fresnel_schlick(max(dot(h, v), 0.0), f0);

    // KHR_materials_anisotropy (direct specular only; IBL stays isotropic).
    let uv_aniso = khr_texture_transform_uv(in.uv0, in.uv1, material.uv_transforms[4u]);
    let aniso_sample = textureSample(aniso_tex, mat_sampler, uv_aniso).rgb;
    let cos_r = cos(material.anisotropy_rotation);
    let sin_r = sin(material.anisotropy_rotation);
    var dir2 = aniso_sample.rg * 2.0 - vec2<f32>(1.0);
    let len_d = length(dir2);
    dir2 = select(vec2<f32>(1.0, 0.0), dir2 / len_d, len_d > 1e-4);
    let rot_m = mat2x2<f32>(cos_r, sin_r, -sin_r, cos_r);
    dir2 = rot_m * dir2;
    let aniso_mag = clamp(material.anisotropy_strength * aniso_sample.b, 0.0, 1.0);

    let tan_w = normalize(in.world_tangent);
    let bit_w = normalize(in.world_bitangent);
    let n_geom = normalize(in.world_normal);
    let t_dir = normalize(tan_w * dir2.x + bit_w * dir2.y);
    let b_dir = normalize(cross(n_geom, t_dir));

    let TdotV = dot(t_dir, v);
    let BdotV = dot(b_dir, v);
    let TdotL = dot(t_dir, l);
    let BdotL = dot(b_dir, l);
    let TdotH = dot(t_dir, h);
    let BdotH = dot(b_dir, h);

    let d_iso = distribution_ggx(n, h, roughness);
    let g_iso = geometry_smith(n, v, l, roughness);
    let at = mix(roughness, 1.0, aniso_mag * aniso_mag);
    let ab = roughness;
    let d_an = d_ggx_aniso(nh, TdotH, BdotH, at, ab);
    let v_an = v_ggx_aniso(nl, nv, BdotV, TdotV, TdotL, BdotL, at, ab);
    let use_aniso = aniso_mag > 0.002;
    let d_eff = select(d_iso, d_an, use_aniso);
    let g_eff = select(g_iso, v_an, use_aniso);
    let specular_base = d_eff * g_eff * f / denom;

    // KHR_materials_clearcoat — lobe additif (direct).
    let d_cc = distribution_ggx(n, h, cc_rough_eff);
    let g_cc = geometry_smith(n, v, l, cc_rough_eff);
    let f_cc = fresnel_schlick(max(dot(h, v), 0.0), f0_coat);
    let spec_coat = d_cc * g_cc * f_cc / denom * cc_f;
    let specular_direct = specular_base + spec_coat;

    let kd_direct       = (vec3<f32>(1.0) - f) * (1.0 - metallic);
    let diffuse_direct  = kd_direct * albedo / PI;
    let shadow_factor   = pcf_shadow(in.world_pos);
    let direct          = (diffuse_direct + specular_direct) * frame.light_color * nl * shadow_factor;

    // ── IBL ambient ───────────────────────────────────────────────────────────
    let ks_ibl       = fresnel_schlick_roughness(max(dot(n, v), 0.0), f0, roughness);
    let kd_ibl       = (vec3<f32>(1.0) - ks_ibl) * (1.0 - metallic);
    let irradiance   = textureSample(irradiance_map, ibl_sampler, n).rgb;
    let diffuse_ibl  = kd_ibl * irradiance * albedo;

    let refl         = reflect(-v, n);
    let max_lod      = 4.0;
    let prefiltered  = textureSampleLevel(prefiltered_map, ibl_sampler, refl, roughness * max_lod).rgb;
    let brdf_uv      = vec2<f32>(clamp(max(dot(n, v), 0.0), 0.001, 1.0), clamp(roughness, 0.0, 1.0));
    let brdf_sample  = textureSample(brdf_lut, ibl_sampler, brdf_uv).rg;
    let specular_ibl = prefiltered * (ks_ibl * brdf_sample.x + brdf_sample.y);

    let ks_cc_ibl = fresnel_schlick_roughness(max(dot(n, v), 0.0), f0_coat, cc_rough_eff);
    let prefiltered_cc = textureSampleLevel(prefiltered_map, ibl_sampler, refl, cc_rough_eff * max_lod).rgb;
    let brdf_cc_uv = vec2<f32>(clamp(max(dot(n, v), 0.0), 0.001, 1.0), clamp(cc_rough_eff, 0.0, 1.0));
    let brdf_cc = textureSample(brdf_lut, ibl_sampler, brdf_cc_uv).rg;
    let spec_ibl_coat = prefiltered_cc * (ks_cc_ibl * brdf_cc.x + brdf_cc.y) * cc_f;

    let ambient = diffuse_ibl + specular_ibl + spec_ibl_coat;
    let color   = ambient + direct + emissive;

    // Output linear HDR — tone mapping is done by the post-process pass.
    return vec4<f32>(color, material.albedo.a * albedo_sample.a);
}
