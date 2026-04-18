// group(0) = FrameUniforms   (per-frame, binding 0)
// group(1) = ObjectUniforms  (per-object, dynamic offset)
// group(2) = MaterialUniforms + textures (per-material)

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
}

struct ObjectUniforms {
    world: mat4x4<f32>,
}

// Scalar multipliers — each is multiplied by the corresponding texture sample.
struct MaterialUniforms {
    albedo:    vec4<f32>,
    emissive:  vec4<f32>,
    metallic:  f32,
    roughness: f32,
    _pad0:     f32,
    _pad1:     f32,
}

@group(0) @binding(0) var<uniform> frame:    FrameUniforms;
@group(1) @binding(0) var<uniform> object:   ObjectUniforms;
@group(2) @binding(0) var<uniform> material: MaterialUniforms;
@group(2) @binding(1) var albedo_tex:  texture_2d<f32>;
@group(2) @binding(2) var normal_tex:  texture_2d<f32>;
@group(2) @binding(3) var mr_tex:      texture_2d<f32>; // G=roughness, B=metallic (glTF spec)
@group(2) @binding(4) var emissive_tex: texture_2d<f32>;
@group(2) @binding(5) var mat_sampler: sampler;

struct VertexInput {
    @location(0) position:  vec3<f32>,
    @location(1) uv0:       vec2<f32>,
    @location(2) uv1:       vec2<f32>,
    @location(3) normal:    vec3<f32>,
    @location(4) tangent:   vec3<f32>,
    @location(5) bitangent: vec3<f32>,
    @location(6) color:     vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_pos:       vec4<f32>,
    @location(0)       world_pos:      vec3<f32>,
    @location(1)       world_normal:   vec3<f32>,
    @location(2)       world_tangent:  vec3<f32>,
    @location(3)       world_bitangent: vec3<f32>,
    @location(4)       uv0:            vec2<f32>,
    @location(5)       color:          vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    let world_pos = object.world * vec4<f32>(in.position, 1.0);
    let normal_mat = mat3x3<f32>(
        object.world[0].xyz,
        object.world[1].xyz,
        object.world[2].xyz,
    );
    var out: VertexOutput;
    out.clip_pos        = frame.projection * frame.view * world_pos;
    out.world_pos       = world_pos.xyz;
    out.world_normal    = normalize(normal_mat * in.normal);
    out.world_tangent   = normalize(normal_mat * in.tangent);
    out.world_bitangent = normalize(normal_mat * in.bitangent);
    out.uv0             = in.uv0;
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

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // ── texture sampling ──────────────────────────────────────────────────────
    let albedo_sample = textureSample(albedo_tex, mat_sampler, in.uv0);
    let mr_sample     = textureSample(mr_tex, mat_sampler, in.uv0);
    let emit_sample   = textureSample(emissive_tex, mat_sampler, in.uv0);

    // glTF: B = metallic, G = roughness; multiply by scalar factors
    let albedo    = material.albedo.rgb * albedo_sample.rgb * in.color.rgb;
    let metallic  = material.metallic  * mr_sample.b;
    let roughness = clamp(material.roughness * mr_sample.g, 0.04, 1.0);
    let emissive  = material.emissive.rgb * emit_sample.rgb;

    // ── normal mapping (TBN) ──────────────────────────────────────────────────
    let normal_sample = textureSample(normal_tex, mat_sampler, in.uv0).xyz;
    let n_tangent = normalize(normal_sample * 2.0 - vec3<f32>(1.0));
    let tbn = mat3x3<f32>(
        normalize(in.world_tangent),
        normalize(in.world_bitangent),
        normalize(in.world_normal),
    );
    let n = normalize(tbn * n_tangent);

    // ── Cook-Torrance BRDF ────────────────────────────────────────────────────
    let v = normalize(frame.camera_position - in.world_pos);
    let l = normalize(-frame.light_direction);
    let h = normalize(v + l);

    let f0 = mix(vec3<f32>(0.04), albedo, metallic);
    let d  = distribution_ggx(n, h, roughness);
    let g  = geometry_smith(n, v, l, roughness);
    let f  = fresnel_schlick(max(dot(h, v), 0.0), f0);

    let nl    = max(dot(n, l), 0.0);
    let denom = 4.0 * max(dot(n, v), 0.0) * nl + 0.0001;
    let specular = d * g * f / denom;

    let kd      = (vec3<f32>(1.0) - f) * (1.0 - metallic);
    let diffuse = kd * albedo / PI;

    let radiance = frame.light_color * nl;
    let direct   = (diffuse + specular) * radiance;
    let ambient  = frame.ambient_intensity * albedo;
    let color    = ambient + direct + emissive;

    // Reinhard tone mapping
    let mapped = color / (color + vec3<f32>(1.0));
    return vec4<f32>(mapped, material.albedo.a * albedo_sample.a);
}
