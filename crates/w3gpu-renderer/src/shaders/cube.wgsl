// group(0): per-frame uniforms   (FrameUniforms)
// group(1): per-object uniforms  (ObjectUniforms, dynamic offset)

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
    _pad2:               vec3<f32>,
}

struct ObjectUniforms {
    world: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> frame:  FrameUniforms;
@group(1) @binding(0) var<uniform> object: ObjectUniforms;

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
    @builtin(position) clip_pos:     vec4<f32>,
    @location(0)       world_normal: vec3<f32>,
    @location(1)       color:        vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    let world_pos = object.world * vec4<f32>(in.position, 1.0);
    // Upper-left 3x3 of world matrix = normal transform (uniform scale assumed)
    let normal_mat = mat3x3<f32>(
        object.world[0].xyz,
        object.world[1].xyz,
        object.world[2].xyz,
    );
    var out: VertexOutput;
    out.clip_pos     = frame.projection * frame.view * world_pos;
    out.world_normal = normalize(normal_mat * in.normal);
    out.color        = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let light    = normalize(-frame.light_direction);
    let diffuse  = max(dot(in.world_normal, light), 0.0);
    let lighting = frame.ambient_intensity + diffuse * (1.0 - frame.ambient_intensity);
    return vec4<f32>(in.color.rgb * frame.light_color * lighting, 1.0);
}
