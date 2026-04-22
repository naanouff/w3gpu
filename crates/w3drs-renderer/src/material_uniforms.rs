/// Per-material GPU uniform — **288** bytes, std140-compatible (base 64 + `UvTransformGpu`×7).
///
/// Bloc tête (64 octets) puis `uv_transforms[7]` à partir de l’offset **64**.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UvTransformGpu {
    pub offset: [f32; 2],
    pub rotation: f32,
    pub _pad0: f32,
    pub scale: [f32; 2],
    pub tex_coord: u32,
    pub _pad1: u32,
}

impl From<w3drs_assets::TextureUvTransform> for UvTransformGpu {
    fn from(t: w3drs_assets::TextureUvTransform) -> Self {
        Self {
            offset: t.offset,
            rotation: t.rotation,
            _pad0: 0.0,
            scale: t.scale,
            tex_coord: t.tex_coord,
            _pad1: 0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialUniforms {
    pub albedo: [f32; 4],
    pub emissive: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub anisotropy_strength: f32,
    pub anisotropy_rotation: f32,
    pub ior: f32,
    pub clearcoat_factor: f32,
    pub clearcoat_roughness: f32,
    pub _pad_main: f32,
    pub uv_transforms: [UvTransformGpu; 7],
}

impl From<&w3drs_assets::Material> for MaterialUniforms {
    fn from(m: &w3drs_assets::Material) -> Self {
        let uv: [UvTransformGpu; 7] =
            std::array::from_fn(|i| UvTransformGpu::from(m.texture_transforms[i]));
        Self {
            albedo: m.albedo,
            emissive: [m.emissive[0], m.emissive[1], m.emissive[2], 0.0],
            metallic: m.metallic,
            roughness: m.roughness,
            anisotropy_strength: m.anisotropy_strength,
            anisotropy_rotation: m.anisotropy_rotation,
            ior: m.ior,
            clearcoat_factor: m.clearcoat_factor,
            clearcoat_roughness: m.clearcoat_roughness,
            _pad_main: 0.0,
            uv_transforms: uv,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MaterialUniforms;

    #[test]
    fn material_uniforms_size_288() {
        assert_eq!(std::mem::size_of::<MaterialUniforms>(), 288);
    }

    #[test]
    fn uv_transform_gpu_is_32_bytes() {
        assert_eq!(std::mem::size_of::<super::UvTransformGpu>(), 32);
    }
}
