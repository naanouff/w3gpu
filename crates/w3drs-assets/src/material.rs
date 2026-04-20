#[derive(Clone, Debug, Default)]
pub enum ShadingModel {
    #[default]
    Pbr,
    Unlit,
}

#[derive(Clone, Debug)]
pub struct Material {
    pub name: String,
    pub shading_model: ShadingModel,
    pub albedo: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
    pub alpha_mode: AlphaMode,
    pub alpha_cutoff: f32,
    pub double_sided: bool,
    /// `KHR_materials_anisotropy` — combined strength (factor × texture blue when present).
    pub anisotropy_strength: f32,
    /// Rotation in radians (CCW in tangent–bitangent plane from tangent).
    pub anisotropy_rotation: f32,
    /// glTF `anisotropyTexture.texCoord` set (0 or 1).
    pub anisotropy_tex_coord: u32,
    /// `KHR_materials_ior` — index of refraction (default **1.5** when extension absent, per Khronos).
    pub ior: f32,
    /// `KHR_materials_clearcoat` — facteur (0–1) ; textures non lues dans cette itération.
    pub clearcoat_factor: f32,
    /// `KHR_materials_clearcoat` — rugosité de la couche (facteur seul).
    pub clearcoat_roughness: f32,
}

#[derive(Clone, Debug, Default)]
pub enum AlphaMode {
    #[default]
    Opaque,
    Mask,
    Blend,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            name: String::new(),
            shading_model: ShadingModel::Pbr,
            albedo: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            emissive: [0.0; 3],
            alpha_mode: AlphaMode::Opaque,
            alpha_cutoff: 0.5,
            double_sided: false,
            anisotropy_strength: 0.0,
            anisotropy_rotation: 0.0,
            anisotropy_tex_coord: 0,
            ior: 1.5,
            clearcoat_factor: 0.0,
            clearcoat_roughness: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_albedo_is_white_opaque() {
        let m = Material::default();
        assert_eq!(m.albedo, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(m.metallic, 0.0);
        assert_eq!(m.roughness, 0.5);
        assert_eq!(m.emissive, [0.0; 3]);
        assert_eq!(m.alpha_cutoff, 0.5);
        assert!(!m.double_sided);
        assert_eq!(m.anisotropy_strength, 0.0);
        assert_eq!(m.anisotropy_rotation, 0.0);
        assert_eq!(m.anisotropy_tex_coord, 0);
        assert!((m.ior - 1.5).abs() < 1e-6);
        assert_eq!(m.clearcoat_factor, 0.0);
        assert_eq!(m.clearcoat_roughness, 0.0);
    }

    #[test]
    fn default_alpha_mode_opaque() {
        let m = Material::default();
        assert!(matches!(m.alpha_mode, AlphaMode::Opaque));
    }

    #[test]
    fn default_shading_pbr() {
        let m = Material::default();
        assert!(matches!(m.shading_model, ShadingModel::Pbr));
    }

    #[test]
    fn default_ior_matches_schlick_f0_04() {
        let m = Material::default();
        let x = (m.ior - 1.0) / (m.ior + 1.0);
        let f0 = x * x;
        assert!((f0 - 0.04).abs() < 1e-5, "IOR 1.5 → F0 ≈ 0.04, got {f0}");
    }
}
