/// Per-material GPU uniform — std140, **512** bytes : en-tête + `UvTransformGpu`×12.
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
    /// `occlusionTexture.strength` (glTF) ; 1.0 = plein effet, sans texture 1×1 (R=1) → facteur 1.0 côté shading.
    pub occlusion_strength: f32,
    /// x = transmission, y = thickness, z = attenuation_distance, w = specular_factor
    pub khr0: [f32; 4],
    /// `specular_color_factor` (linéaire) + pad
    pub khr1: [f32; 4],
    /// `attenuation_color` + pad
    pub khr2: [f32; 4],
    /// bit0 KHR specular, bit1 transmission, bit2 volume
    pub khr_flags: u32,
    pub normal_scale: f32,
    pub alpha_cutoff: f32,
    /// 0 opaque, 1 mask, 2 blend.
    pub alpha_mode: u32,
    pub uv_transforms: [UvTransformGpu; 12],
}

impl From<&w3drs_assets::Material> for MaterialUniforms {
    fn from(m: &w3drs_assets::Material) -> Self {
        let uv: [UvTransformGpu; 12] =
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
            occlusion_strength: m.occlusion_strength,
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
            normal_scale: m.normal_scale,
            alpha_cutoff: m.alpha_cutoff,
            alpha_mode: match &m.alpha_mode {
                w3drs_assets::AlphaMode::Opaque => 0,
                w3drs_assets::AlphaMode::Mask => 1,
                w3drs_assets::AlphaMode::Blend => 2,
            },
            uv_transforms: uv,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MaterialUniforms;
    use w3drs_assets::{AlphaMode, Material};

    #[test]
    fn material_uniforms_size_512() {
        assert_eq!(std::mem::size_of::<MaterialUniforms>(), 512);
    }

    #[test]
    fn uv_transform_gpu_is_32_bytes() {
        assert_eq!(std::mem::size_of::<super::UvTransformGpu>(), 32);
    }

    #[test]
    fn material_uniforms_carries_alpha_and_normal_controls() {
        let mut material = Material::default();
        material.alpha_mode = AlphaMode::Mask;
        material.alpha_cutoff = 0.42;
        material.normal_scale = 0.5;

        let uniforms = MaterialUniforms::from(&material);
        assert_eq!(uniforms.alpha_mode, 1);
        assert!((uniforms.alpha_cutoff - 0.42).abs() < 1e-6);
        assert!((uniforms.normal_scale - 0.5).abs() < 1e-6);
    }
}
