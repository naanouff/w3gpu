/// Per-material GPU uniform — 64 bytes, std140-compatible.
///
/// WGSL layout:
///   albedo:    vec4<f32>   offset  0
///   emissive:  vec4<f32>   offset 16
///   metallic:  f32         offset 32
///   roughness: f32         offset 36
///   anisotropy_strength / rotation — 40, 44
///   anisotropy_tex_coord (u32) / ior (f32) — 48, 52
///   _pad_tail  u64         offset 56 → total 64
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialUniforms {
    pub albedo: [f32; 4],
    pub emissive: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub anisotropy_strength: f32,
    pub anisotropy_rotation: f32,
    pub anisotropy_tex_coord: u32,
    pub ior: f32,
    pub _pad_tail: u64,
}

impl From<&w3drs_assets::Material> for MaterialUniforms {
    fn from(m: &w3drs_assets::Material) -> Self {
        Self {
            albedo: m.albedo,
            emissive: [m.emissive[0], m.emissive[1], m.emissive[2], 0.0],
            metallic: m.metallic,
            roughness: m.roughness,
            anisotropy_strength: m.anisotropy_strength,
            anisotropy_rotation: m.anisotropy_rotation,
            anisotropy_tex_coord: m.anisotropy_tex_coord,
            ior: m.ior,
            _pad_tail: 0,
        }
    }
}
