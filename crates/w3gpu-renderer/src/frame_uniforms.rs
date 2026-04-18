/// Per-frame GPU uniform block.
/// Layout matches w3dts ShaderStructs.ts FrameUniformsLayout byte-for-byte.
/// All vec3 fields have explicit f32 padding to maintain std140 16-byte alignment.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FrameUniforms {
    pub projection:          [[f32; 4]; 4],
    pub view:                [[f32; 4]; 4],
    pub inv_view_projection: [[f32; 4]; 4],
    pub camera_position:     [f32; 3],
    pub _pad0:               f32,
    pub light_direction:     [f32; 3],
    pub _pad1:               f32,
    pub light_color:         [f32; 3],
    pub ambient_intensity:   f32,
    pub total_time:          f32,
    pub _pad2:               [f32; 3],
}
