/// Per-material GPU uniform — 48 bytes, std140-compatible.
///
/// WGSL layout:
///   albedo:    vec4<f32>   offset  0
///   emissive:  vec4<f32>   offset 16  (w unused)
///   metallic:  f32         offset 32
///   roughness: f32         offset 36
///   _pad0/1:   f32×2       offset 40  → total 48
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialUniforms {
    pub albedo:    [f32; 4],
    pub emissive:  [f32; 4],
    pub metallic:  f32,
    pub roughness: f32,
    pub _pad:      [f32; 2],
}

impl From<&w3gpu_assets::Material> for MaterialUniforms {
    fn from(m: &w3gpu_assets::Material) -> Self {
        Self {
            albedo:    m.albedo,
            emissive:  [m.emissive[0], m.emissive[1], m.emissive[2], 0.0],
            metallic:  m.metallic,
            roughness: m.roughness,
            _pad:      [0.0; 2],
        }
    }
}
