/// Per-frame light data uploaded to the GPU for shadow mapping.
/// Layout: 80 bytes (std140 compatible).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniforms {
    pub view_proj: [[f32; 4]; 4], // offset  0 — light-space VP matrix
    pub shadow_bias: f32,         // offset 64
    pub _pad: [f32; 3],           // offset 68
} // total:  80 bytes
