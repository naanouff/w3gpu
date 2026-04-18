use glam::Vec3;

#[derive(Clone, Debug)]
pub struct DirectionalLightComponent {
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub cast_shadow: bool,
}

impl Default for DirectionalLightComponent {
    fn default() -> Self {
        Self {
            direction: Vec3::new(-0.5, -1.0, -0.5).normalize(),
            color: Vec3::ONE,
            intensity: 1.0,
            cast_shadow: true,
        }
    }
}
