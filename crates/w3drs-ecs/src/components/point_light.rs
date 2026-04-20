use glam::Vec3;

#[derive(Clone, Debug)]
pub struct PointLightComponent {
    pub color: Vec3,
    pub intensity: f32,
    pub range: f32,
}

impl Default for PointLightComponent {
    fn default() -> Self {
        Self { color: Vec3::ONE, intensity: 1.0, range: 10.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let l = PointLightComponent::default();
        assert_eq!(l.intensity, 1.0);
        assert_eq!(l.range, 10.0);
        assert_eq!(l.color, glam::Vec3::ONE);
    }
}
