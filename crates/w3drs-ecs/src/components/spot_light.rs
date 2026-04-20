use glam::Vec3;

#[derive(Clone, Debug)]
pub struct SpotLightComponent {
    pub color: Vec3,
    pub intensity: f32,
    pub range: f32,
    pub inner_cone_angle: f32,
    pub outer_cone_angle: f32,
}

impl Default for SpotLightComponent {
    fn default() -> Self {
        Self {
            color: Vec3::ONE,
            intensity: 1.0,
            range: 10.0,
            inner_cone_angle: 20.0_f32.to_radians(),
            outer_cone_angle: 30.0_f32.to_radians(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cone_angles_are_radians() {
        let l = SpotLightComponent::default();
        assert!(l.inner_cone_angle < l.outer_cone_angle);
        assert!(l.outer_cone_angle < std::f32::consts::FRAC_PI_2);
    }
}
