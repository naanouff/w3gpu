// group(0) = FrameUniforms              (per-frame, includes shadow light_view_proj)
// group(1) = instances storage buffer  (array<mat4x4<f32>>, indexed by instance_index)
// group(2) = MaterialUniforms + textures (per-material)
// group(3) = IBL (bindings 0-3) + shadow map/sampler (bindings 4-5)
//
// PBR fragment aligné sur w3dts :
//   `packages/viewer-editor/public/shaders/shared/pbr_functions.wgsl`
//   `packages/viewer-editor/public/shaders/graph_templates/pbr_master_node.wgsl`
// (lumière directe = directionnelle unique + ombre PCF ; extensions w3drs : clearcoat,
//  `ibl_flags` / `ibl_diffuse_scale`, LOD préfiltre borné comme w3dts pour l’échantillon.)

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
    light_view_proj:     mat4x4<f32>,
    shadow_bias:         f32,
    ibl_flags:            u32,
    ibl_diffuse_scale:   f32,
    _pad3:               f32,
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
    emissive:  vec4<f32>, // rgb + emissive_strength (w)
    metallic:  f32,
    roughness: f32,
    anisotropy_strength: f32,
    anisotropy_rotation: f32,
    ior:       f32,
    clearcoat_factor:     f32,
    clearcoat_roughness:  f32,
    _pad_main: f32,
    khr0:       vec4<f32>, // transmission, thickness, atten dist, specular factor
    khr1:       vec4<f32>, // specular color factor + 0
    khr2:       vec4<f32>, // attenuation color + 0
    khr_flags:  u32,
    _kf0: u32, _kf1: u32, _kf2: u32,
    uv_transforms: array<UvTransform, 11>,
}

@group(0) @binding(0) var<uniform>        frame:     FrameUniforms;
@group(1) @binding(0) var<storage, read>  instances: array<mat4x4<f32>>;
@group(2) @binding(0) var<uniform>        material:  MaterialUniforms;
@group(2) @binding(1) var albedo_tex:    texture_2d<f32>;
@group(2) @binding(2) var normal_tex:    texture_2d<f32>;
@group(2) @binding(3) var mr_tex:        texture_2d<f32>;
@group(2) @binding(4) var emissive_tex:  texture_2d<f32>;
@group(2) @binding(5) var aniso_tex:            texture_2d<f32>;
@group(2) @binding(6) var clearcoat_tex:        texture_2d<f32>;
@group(2) @binding(7) var clearcoat_rough_tex:  texture_2d<f32>;
@group(2) @binding(8) var mat_sampler:          sampler;
@group(2) @binding(9)  var trans_tex:     texture_2d<f32>;
@group(2) @binding(10) var specular_tex:  texture_2d<f32>;
@group(2) @binding(11) var spec_color_tex: texture_2d<f32>;
@group(2) @binding(12) var thick_tex:     texture_2d<f32>;

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

// ═══════════════════════════════════════════════════════════════════════════════
// w3dts `pbr_functions.wgsl` (noms et formules identiques)
// ═══════════════════════════════════════════════════════════════════════════════

const PI: f32 = 3.14159265358979;

fn DistributionGGX(N: vec3<f32>, H: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let NdotH = max(dot(N, H), 0.0);
    let NdotH2 = NdotH * NdotH;
    let nom = a2;
    var denom = (NdotH2 * (a2 - 1.0) + 1.0);
    denom = PI * denom * denom;
    return nom / max(denom, 0.0000001);
}

fn GeometrySchlickGGX(NdotV: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    let nom = NdotV;
    let denom = NdotV * (1.0 - k) + k;
    return nom / denom;
}

fn GeometrySmith4(N: vec3<f32>, V: vec3<f32>, L: vec3<f32>, roughness: f32) -> f32 {
    let NdotV = max(dot(N, V), 0.0);
    let NdotL = max(dot(N, L), 0.0);
    let ggx2 = GeometrySchlickGGX(NdotV, roughness);
    let ggx1 = GeometrySchlickGGX(NdotL, roughness);
    return ggx1 * ggx2;
}

fn fresnelSchlick(cosTheta: f32, F0: vec3<f32>) -> vec3<f32> {
    return F0 + (vec3<f32>(1.0) - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

fn fresnelSchlickRoughness(cosTheta: f32, F0: vec3<f32>, roughness: f32) -> vec3<f32> {
    return F0 + (max(vec3<f32>(1.0 - roughness), F0) - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

fn D_GGX_Anisotropic(NdotH: f32, H: vec3<f32>, T: vec3<f32>, B: vec3<f32>, at: f32, ab: f32) -> f32 {
    let TdotH = dot(T, H);
    let BdotH = dot(B, H);
    let a2 = at * ab;
    let d = vec3<f32>(ab * TdotH, at * BdotH, a2 * NdotH);
    let d2 = dot(d, d);
    let b2 = a2 / d2;
    return a2 * b2 * b2 * 0.31830988618;
}

fn V_SmithGGXCorrelated_Anisotropic(
    at: f32,
    ab: f32,
    TdotV: f32,
    BdotV: f32,
    TdotL: f32,
    BdotL: f32,
    NdotV: f32,
    NdotL: f32,
) -> f32 {
    let GGXV = NdotL * length(vec3<f32>(at * TdotV, ab * BdotV, NdotV));
    let GGXL = NdotV * length(vec3<f32>(at * TdotL, ab * BdotL, NdotL));
    let v = 0.5 / (GGXV + GGXL);
    return clamp(v, 0.0, 1.0);
}

/// w3dts `pbr_master_node` : F0 diélectrique depuis IOR (pas d’extension specular w3drs).
fn dielectric_f0_from_ior(ior: f32) -> vec3<f32> {
    let ior_safe = max(ior, 1.001);
    let f = pow((ior_safe - 1.0) / (ior_safe + 1.0), 2.0);
    return vec3<f32>(f, f, f);
}

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

fn pcf_shadow(world_pos: vec3<f32>) -> f32 {
    let light_clip  = frame.light_view_proj * vec4<f32>(world_pos, 1.0);
    let ndc         = light_clip.xyz / light_clip.w;
    let uv          = ndc.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
    let depth_ref   = ndc.z - frame.shadow_bias;
    let in_frustum  = uv.x >= 0.0 && uv.x <= 1.0 && uv.y >= 0.0 && uv.y <= 1.0 && depth_ref <= 1.0;
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
    let emissive  = material.emissive.rgb * emit_sample.rgb * material.emissive.w;

    let normal_sample = textureSample(normal_tex, mat_sampler, uv_n).xyz;
    let n_tangent = normalize(normal_sample * 2.0 - vec3<f32>(1.0));
    let tbn = mat3x3<f32>(
        normalize(in.world_tangent),
        normalize(in.world_bitangent),
        normalize(in.world_normal),
    );
    let N = normalize(tbn * n_tangent);

    let uv_cc = khr_texture_transform_uv(in.uv0, in.uv1, material.uv_transforms[5u]);
    let uv_cr = khr_texture_transform_uv(in.uv0, in.uv1, material.uv_transforms[6u]);
    let cc_tex_r   = textureSample(clearcoat_tex, mat_sampler, uv_cc).r;
    let cc_rough_g = textureSample(clearcoat_rough_tex, mat_sampler, uv_cr).g;
    let cc_f = clamp(material.clearcoat_factor * cc_tex_r, 0.0, 1.0);
    let cc_rough_eff = max(material.clearcoat_roughness * cc_rough_g, 0.089);
    let f0_coat = dielectric_f0_from_ior(1.5);

    let V = normalize(frame.camera_position - in.world_pos);
    let L = normalize(-frame.light_direction);
    let H = normalize(V + L);
    let NdotV = max(dot(N, V), 0.001);

    // ── TBN anisotrope (w3dts `pbr_master_node` §4) ───────────────────────────
    var T_interp = normalize(in.world_tangent);
    var B_interp = normalize(in.world_bitangent);
    var T = normalize(T_interp - dot(T_interp, N) * N);
    let handedness = select(-1.0, 1.0, dot(cross(N, T), B_interp) >= 0.0);
    var B = normalize(cross(N, T)) * handedness;

    let uv_aniso = khr_texture_transform_uv(in.uv0, in.uv1, material.uv_transforms[4u]);
    let anisoTex = textureSample(aniso_tex, mat_sampler, uv_aniso).rgb;
    let anisoDirectionTex = anisoTex.rg * 2.0 - 1.0;
    let anisotropyStrength = clamp(material.anisotropy_strength, 0.0, 1.0);
    let anisotropyRotation = material.anisotropy_rotation;
    let anisotropy = anisotropyStrength * anisoTex.b;

    if (anisotropy != 0.0) {
        let cosRot = cos(anisotropyRotation);
        let sinRot = sin(anisotropyRotation);
        let rotMat = mat2x2<f32>(vec2<f32>(cosRot, sinRot), vec2<f32>(-sinRot, cosRot));
        let dirLen = length(anisoDirectionTex);
        let dirParams = select(vec2<f32>(1.0, 0.0), anisoDirectionTex / dirLen, dirLen > 1e-5);
        let finalDir = rotMat * dirParams;
        let rotT = T * finalDir.x + B * finalDir.y;
        let rotB = B * finalDir.x - T * finalDir.y;
        T = normalize(rotT);
        B = normalize(rotB);
    }

    let alphaRoughness = roughness * roughness;
    let aniMix = clamp(anisotropy, 0.0, 1.0);
    let at0 = mix(alphaRoughness, 1.0, aniMix * aniMix);
    let ab0 = alphaRoughness;
    let at = max(at0, 1e-4);
    let ab = max(ab0, 1e-4);

    // ── KHR `specular` F0 (diélectrique) + Direct (w3dts §5) ─────────────────
    var f0_dielec = dielectric_f0_from_ior(material.ior);
    if ((material.khr_flags & 1u) != 0u) {
        let uv_sp = khr_texture_transform_uv(in.uv0, in.uv1, material.uv_transforms[8u]);
        let uv_sc = khr_texture_transform_uv(in.uv0, in.uv1, material.uv_transforms[9u]);
        let s_w = material.khr0.w * textureSample(specular_tex, mat_sampler, uv_sp).a;
        let c_tex = textureSample(spec_color_tex, mat_sampler, uv_sc).rgb;
        f0_dielec = min(material.khr1.xyz * c_tex * s_w, vec3<f32>(1.0));
    }
    let f0 = mix(f0_dielec, albedo, metallic);
    let NdotL = max(dot(N, L), 0.0);
    var specular_base = vec3<f32>(0.0);
    var diffuse_direct = vec3<f32>(0.0);

    if (NdotL > 0.0) {
        let H_loop = H;
        let NdotH = max(dot(N, H_loop), 0.0);
        let VdotH = max(dot(V, H_loop), 0.0);

        var D: f32;
        var G: f32;
        if (anisotropy != 0.0) {
            D = D_GGX_Anisotropic(NdotH, H_loop, T, B, at, ab);
            G = V_SmithGGXCorrelated_Anisotropic(
                at,
                ab,
                dot(T, V),
                dot(B, V),
                dot(T, L),
                dot(B, L),
                NdotV,
                NdotL,
            );
        } else {
            D = DistributionGGX(N, H_loop, roughness);
            G = GeometrySmith4(N, V, L, roughness);
        }

        let F = fresnelSchlick(VdotH, f0);
        let kS = F;
        var kD = vec3<f32>(1.0) - kS;
        kD *= (1.0 - metallic);

        let spec_scalar = select(
            (D * G) / (4.0 * NdotV * NdotL + 0.0001),
            0.0,
            roughness >= 0.999,
        );
        specular_base = spec_scalar * F;

        // Clearcoat (même microfacet iso que w3dts pour le base coat ; lobe additif w3drs)
        let d_cc = DistributionGGX(N, H_loop, cc_rough_eff);
        let g_cc = GeometrySmith4(N, V, L, cc_rough_eff);
        let f_cc = fresnelSchlick(VdotH, f0_coat);
        let spec_cc = select(
            (d_cc * g_cc) / (4.0 * NdotV * NdotL + 0.0001),
            0.0,
            cc_rough_eff >= 0.999,
        );
        specular_base = specular_base + spec_cc * f_cc * cc_f;

        diffuse_direct = (kD * albedo / PI) * frame.light_color * NdotL;
        specular_base = specular_base * frame.light_color * NdotL;
    }

    let shadow_factor = pcf_shadow(in.world_pos);
    let direct = (diffuse_direct + specular_base) * shadow_factor;

    // ── IBL (w3dts §6, sans transmission / AO texture) ──────────────────────────
    let kS_IBL = select(
        fresnelSchlickRoughness(NdotV, f0, roughness),
        vec3<f32>(0.0),
        roughness >= 0.999,
    );
    let kD_IBL = (vec3<f32>(1.0) - kS_IBL) * (1.0 - metallic);

    var irradiance = textureSample(irradiance_map, ibl_sampler, N).rgb;
    if ((frame.ibl_flags & 1u) != 0u) {
        irradiance = vec3<f32>(0.0);
    }
    let diffuseIBL = irradiance * albedo;
    let diffuseIBL_surf = kD_IBL * diffuseIBL * frame.ibl_diffuse_scale;

    var R_vec: vec3<f32>;
    if (anisotropy != 0.0) {
        var bentNormal = cross(B, V);
        bentNormal = normalize(cross(bentNormal, B));
        let term = 1.0 - anisotropy * (1.0 - roughness);
        let a_mix = term * term * term * term;
        bentNormal = normalize(mix(bentNormal, N, a_mix));
        R_vec = reflect(-V, bentNormal);
        R_vec = normalize(mix(R_vec, bentNormal, roughness * roughness));
    } else {
        R_vec = reflect(-V, N);
        R_vec = normalize(mix(R_vec, N, roughness * roughness));
    }

    const MAX_REFLECTION_LOD: f32 = 4.0;
    let prefilteredColor =
        textureSampleLevel(prefiltered_map, ibl_sampler, R_vec, roughness * MAX_REFLECTION_LOD).rgb;
    let brdf = textureSample(brdf_lut, ibl_sampler, vec2<f32>(NdotV, roughness)).rg;
    let specularIBL = select(
        prefilteredColor * (kS_IBL * brdf.x + brdf.y),
        vec3<f32>(0.0),
        roughness >= 0.999,
    );

    let ks_cc_ibl = fresnelSchlickRoughness(NdotV, f0_coat, cc_rough_eff);
    let prefiltered_cc =
        textureSampleLevel(prefiltered_map, ibl_sampler, R_vec, cc_rough_eff * MAX_REFLECTION_LOD).rgb;
    let brdf_cc = textureSample(brdf_lut, ibl_sampler, vec2<f32>(NdotV, cc_rough_eff)).rg;
    let spec_ibl_coat = select(
        prefiltered_cc * (ks_cc_ibl * brdf_cc.x + brdf_cc.y) * cc_f,
        vec3<f32>(0.0),
        cc_rough_eff >= 0.999,
    );

    var ambient = diffuseIBL_surf + specularIBL + spec_ibl_coat;
    // KHR transmission + volume (approx. IBL « à travers » + Beer)
    // Pas de branchement sur des valeurs issues d’un `textureSample` (WGSL: flux de contrôle uniforme).
    if ((material.khr_flags & 2u) != 0u) {
        let uv_t = khr_texture_transform_uv(in.uv0, in.uv1, material.uv_transforms[7u]);
        let t_amt = material.khr0.x * textureSample(trans_tex, mat_sampler, uv_t).r;
        var att = vec3<f32>(1.0);
        if ((material.khr_flags & 4u) != 0u) {
            let uv_tk = khr_texture_transform_uv(in.uv0, in.uv1, material.uv_transforms[10u]);
            let th = max(material.khr0.y * textureSample(thick_tex, mat_sampler, uv_tk).g, 0.0);
            let d = max(material.khr0.z, 0.0001);
            att = exp(-(th / d) * (vec3<f32>(1.0) - material.khr2.xyz));
        }
        let t_mix = t_amt * (1.0 - metallic);
        let behind = textureSampleLevel(
            prefiltered_map,
            ibl_sampler,
            -R_vec,
            roughness * MAX_REFLECTION_LOD,
        ).rgb;
        let w = min(max(t_mix, 0.0), 1.0);
        ambient = ambient * (1.0 - w) + behind * albedo * att * w;
    }

    let color = ambient + direct + emissive;
    return vec4<f32>(color, material.albedo.a * albedo_sample.a);
}
