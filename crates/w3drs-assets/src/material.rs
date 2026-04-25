#[derive(Clone, Debug, Default)]
pub enum ShadingModel {
    #[default]
    Pbr,
    Unlit,
}

/// `KHR_texture_transform` + choix de jeu de coordonnées (glTF `texCoord`, 0 ou 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextureUvTransform {
    pub offset: [f32; 2],
    pub scale: [f32; 2],
    /// Radians, sens **anti-horaire** sur les UV (Khronos).
    pub rotation: f32,
    /// Jeu `TEXCOORD_n` effectif (0 ou 1) — peut être surchargé par l’extension sur la `textureInfo`.
    pub tex_coord: u32,
}

impl Default for TextureUvTransform {
    fn default() -> Self {
        Self {
            offset: [0.0, 0.0],
            scale: [1.0, 1.0],
            rotation: 0.0,
            tex_coord: 0,
        }
    }
}

/// Indices des entrées `Material::texture_transforms[]` (alignés shader / GPU).
pub const TEX_UV_ALBEDO: usize = 0;
pub const TEX_UV_NORMAL: usize = 1;
pub const TEX_UV_METALLIC_ROUGHNESS: usize = 2;
pub const TEX_UV_EMISSIVE: usize = 3;
pub const TEX_UV_ANISOTROPY: usize = 4;
pub const TEX_UV_CLEARCOAT: usize = 5;
pub const TEX_UV_CLEARCOAT_ROUGHNESS: usize = 6;
/// `KHR_materials_transmission` (canal R).
pub const TEX_UV_TRANSMISSION: usize = 7;
/// `KHR_materials_specular` (canal A = force spéculaire).
pub const TEX_UV_SPECULAR: usize = 8;
/// `KHR_materials_specular` `specularColorTexture` (sRGB en asset — upload SRGB côté GPU).
pub const TEX_UV_SPECULAR_COLOR: usize = 9;
/// `KHR_materials_volume` `thicknessTexture` (canal G).
pub const TEX_UV_THICKNESS: usize = 10;
/// `occlusionTexture` glTF (canal **R**, linéaire) — souvent le même fichier que M/R, UV distincts.
pub const TEX_UV_OCCLUSION: usize = 11;

/// Nombre d’emplacements `texture_transforms` (0..=11).
pub const MATERIAL_TEX_SLOT_COUNT: usize = 12;

#[derive(Clone, Debug)]
pub struct Material {
    pub name: String,
    pub shading_model: ShadingModel,
    pub albedo: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
    /// glTF `normalTexture.scale` (default 1.0).
    pub normal_scale: f32,
    pub alpha_mode: AlphaMode,
    pub alpha_cutoff: f32,
    pub double_sided: bool,
    /// `KHR_materials_anisotropy` — combined strength (factor × texture blue when present).
    pub anisotropy_strength: f32,
    /// Rotation in radians (CCW in tangent–bitangent plane from tangent).
    pub anisotropy_rotation: f32,
    /// `KHR_materials_ior` — index of refraction (default **1.5** when extension absent, per Khronos).
    pub ior: f32,
    /// `KHR_materials_clearcoat` — facteur (0–1) ; multiplié par le canal **R** de la texture si présente.
    pub clearcoat_factor: f32,
    /// `KHR_materials_clearcoat` — rugosité de la couche ; multipliée par le canal **G** de la texture roughness si présente.
    pub clearcoat_roughness: f32,
    /// `KHR_materials_emissive_strength` — facteur sur l’émissif (défaut 1.0).
    pub emissive_strength: f32,
    /// `KHR_materials_transmission` (facteur, multiplié par R de la texture si présente).
    pub transmission_factor: f32,
    /// `KHR_materials_specular` — poids (× texture A).
    pub specular_factor: f32,
    /// `KHR_materials_specular` — F0 entrant diélectrique, linéaire RGB.
    pub specular_color_factor: [f32; 3],
    /// `KHR_materials_volume` — épaisseur de base (× texture G).
    pub thickness_factor: f32,
    /// `KHR_materials_volume` — distance moyenne d’atténuation.
    pub attenuation_distance: f32,
    /// `KHR_materials_volume` — couleur d’atténuation.
    pub attenuation_color: [f32; 3],
    /// `occlusionTexture.strength` glTF (0–1, défaut 1) ; sans texture, la teinte 1×1 (R=1) n’applique pas d’occlusion.
    pub occlusion_strength: f32,
    /// Drapeaux : bit 0 = KHR specular, 1 = transmission, 2 = volume (attenuation / épaisseur).
    pub khr_flags: u32,
    /// `KHR_texture_transform` + `texCoord` par slot texture (ordre [`TEX_UV_*`](crate::material)).
    pub texture_transforms: [TextureUvTransform; MATERIAL_TEX_SLOT_COUNT],
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
            normal_scale: 1.0,
            alpha_mode: AlphaMode::Opaque,
            alpha_cutoff: 0.5,
            double_sided: false,
            anisotropy_strength: 0.0,
            anisotropy_rotation: 0.0,
            ior: 1.5,
            clearcoat_factor: 0.0,
            clearcoat_roughness: 0.0,
            emissive_strength: 1.0,
            transmission_factor: 0.0,
            specular_factor: 1.0,
            specular_color_factor: [1.0, 1.0, 1.0],
            thickness_factor: 0.0,
            attenuation_distance: 1.0e10,
            attenuation_color: [1.0, 1.0, 1.0],
            occlusion_strength: 1.0,
            khr_flags: 0,
            texture_transforms: [TextureUvTransform::default(); MATERIAL_TEX_SLOT_COUNT],
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
        assert!((m.normal_scale - 1.0).abs() < 1e-5);
        assert_eq!(m.alpha_cutoff, 0.5);
        assert!(!m.double_sided);
        assert_eq!(m.anisotropy_strength, 0.0);
        assert_eq!(m.anisotropy_rotation, 0.0);
        assert!((m.ior - 1.5).abs() < 1e-6);
        assert_eq!(m.clearcoat_factor, 0.0);
        assert_eq!(m.clearcoat_roughness, 0.0);
        assert!((m.emissive_strength - 1.0).abs() < 1e-5);
        assert_eq!(m.transmission_factor, 0.0);
        assert!((m.specular_factor - 1.0).abs() < 1e-5);
        assert!((m.occlusion_strength - 1.0).abs() < 1e-5);
        for t in m.texture_transforms {
            assert_eq!(t, TextureUvTransform::default());
        }
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
