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
        }
    }
}
