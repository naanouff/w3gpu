/// Bit `1` : pas de **diffuse IBL** depuis la cubemap d’irradiance (`irradiance_map` → noir).
pub const IBL_FLAG_DISABLE_IRRADIANCE_DIFFUSE: u32 = 1;

/// Per-frame GPU uniform block — 336 bytes (std140).
/// All vec3 fields carry explicit f32 padding to maintain 16-byte alignment.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FrameUniforms {
    pub projection: [[f32; 4]; 4],          // offset   0
    pub view: [[f32; 4]; 4],                // offset  64
    pub inv_view_projection: [[f32; 4]; 4], // offset 128
    pub camera_position: [f32; 3],          // offset 192
    pub _pad0: f32,                         // offset 204
    pub light_direction: [f32; 3],          // offset 208
    pub _pad1: f32,                         // offset 220
    pub light_color: [f32; 3],              // offset 224
    pub ambient_intensity: f32,             // offset 236
    pub total_time: f32,                    // offset 240
    pub _pad2: [f32; 3],                    // offset 244
    // shadow data (folded in to stay within max_bind_groups = 4)
    pub light_view_proj: [[f32; 4]; 4], // offset 256
    pub shadow_bias: f32,               // offset 320
    pub ibl_flags: u32,                 // offset 324 — voir [`IBL_FLAG_DISABLE_IRRADIANCE_DIFFUSE`]
    /// Atténuation du **diffuse IBL** (carte d’irradiance × `albedo` × `kd_ibl`). `1` = neutre.
    pub ibl_diffuse_scale: f32,          // offset 328
    pub _pad3: f32,                     // offset 332
} // total: 336 bytes
