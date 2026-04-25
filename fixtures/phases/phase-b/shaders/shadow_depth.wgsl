// Shadow depth pass — renders scene geometry from the light's point of view.
// group(0): LightUniforms             (VERTEX)
// group(1): instances storage buffer  (array<mat4x4<f32>>, indexed by instance_index)

struct LightUniforms {
    view_proj:   mat4x4<f32>,
    shadow_bias: f32,
    _pad0: f32, _pad1: f32, _pad2: f32,
}

@group(0) @binding(0) var<uniform>       light:     LightUniforms;
@group(1) @binding(0) var<storage, read> instances: array<mat4x4<f32>>;

@vertex
fn vs_main(
    @builtin(instance_index) inst_idx: u32,
    @location(0) position: vec3<f32>,
) -> @builtin(position) vec4<f32> {
    return light.view_proj * instances[inst_idx] * vec4<f32>(position, 1.0);
}
