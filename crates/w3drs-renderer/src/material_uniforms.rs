/// Per-material GPU uniform — std140, **480** bytes : en-tête + `UvTransformGpu`×11.
///
/// Champs KHR (transmission, specular, volume) : `khr0`..`khr2` + `khr_flags`.
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

/// Aligné `pbr.wgsl` `MaterialUniforms` (même ordre).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialUniforms {
    pub albedo: [f32; 4],
    /// rgb = emissive, **a** = `emissive_strength` (`KHR_materials_emissive_strength`).
    pub emissive: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub anisotropy_strength: f32,
    pub anisotropy_rotation: f32,
    pub ior: f32,
    pub clearcoat_factor: f32,
    pub clearcoat_roughness: f32,
    pub _pad_main: f32,
    /// x = transmission, y = thickness, z = attenuation_distance, w = specular_factor
    pub khr0: [f32; 4],
    /// `specular_color_factor` (linéaire) + pad
    pub khr1: [f32; 4],
    /// `attenuation_color` + pad
    pub khr2: [f32; 4],
    /// bit0 KHR specular, bit1 transmission, bit2 volume
    pub khr_flags: u32,
    pub _kf0: u32,
    pub _kf1: u32,
    pub _kf2: u32,
    pub uv_transforms: [UvTransformGpu; 11],
}

impl From<&w3drs_assets::Material> for MaterialUniforms {
    fn from(m: &w3drs_assets::Material) -> Self {
        let uv: [UvTransformGpu; 11] =
            std::array::from_fn(|i| UvTransformGpu::from(m.texture_transforms[i]));
        Self {
            albedo: m.albedo,
            emissive: [
                m.emissive[0],
                m.emissive[1],
                m.emissive[2],
                m.emissive_strength,
            ],
            metallic: m.metallic,
            roughness: m.roughness,
            anisotropy_strength: m.anisotropy_strength,
            anisotropy_rotation: m.anisotropy_rotation,
            ior: m.ior,
            clearcoat_factor: m.clearcoat_factor,
            clearcoat_roughness: m.clearcoat_roughness,
            _pad_main: 0.0,
            khr0: [
                m.transmission_factor,
                m.thickness_factor,
                m.attenuation_distance,
                m.specular_factor,
            ],
            khr1: [
                m.specular_color_factor[0],
                m.specular_color_factor[1],
                m.specular_color_factor[2],
                0.0,
            ],
            khr2: [
                m.attenuation_color[0],
                m.attenuation_color[1],
                m.attenuation_color[2],
                0.0,
            ],
            khr_flags: m.khr_flags,
            _kf0: 0,
            _kf1: 0,
            _kf2: 0,
            uv_transforms: uv,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MaterialUniforms;

    #[test]
    fn material_uniforms_size_480() {
        assert_eq!(std::mem::size_of::<MaterialUniforms>(), 480);
    }

    #[test]
    fn uv_transform_gpu_is_32_bytes() {
        assert_eq!(std::mem::size_of::<super::UvTransformGpu>(), 32);
    }
}
