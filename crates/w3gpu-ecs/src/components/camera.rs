use glam::Mat4;

#[derive(Clone, Debug)]
pub struct CameraComponent {
    pub fov_y_radians: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
    pub view_matrix: Mat4,
    pub projection_matrix: Mat4,
    pub view_projection_matrix: Mat4,
    pub is_active: bool,
}

impl CameraComponent {
    pub fn new(fov_y_degrees: f32, aspect: f32, near: f32, far: f32) -> Self {
        let fov = fov_y_degrees.to_radians();
        let proj = Mat4::perspective_rh(fov, aspect, near, far);
        Self {
            fov_y_radians: fov,
            aspect,
            near,
            far,
            view_matrix: Mat4::IDENTITY,
            projection_matrix: proj,
            view_projection_matrix: proj,
            is_active: true,
        }
    }
}

impl Default for CameraComponent {
    fn default() -> Self {
        Self::new(60.0, 16.0 / 9.0, 0.1, 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sets_active_and_fov() {
        let c = CameraComponent::new(90.0, 1.0, 0.1, 100.0);
        assert!(c.is_active);
        assert!((c.fov_y_radians - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        assert_eq!(c.near, 0.1);
        assert_eq!(c.far, 100.0);
        assert_eq!(c.aspect, 1.0);
    }

    #[test]
    fn default_camera_is_active() {
        let c = CameraComponent::default();
        assert!(c.is_active);
    }

    #[test]
    fn projection_matrix_not_identity() {
        let c = CameraComponent::new(60.0, 16.0 / 9.0, 0.1, 1000.0);
        assert_ne!(c.projection_matrix, Mat4::IDENTITY);
    }

    #[test]
    fn view_matrix_starts_identity() {
        let c = CameraComponent::default();
        assert_eq!(c.view_matrix, Mat4::IDENTITY);
    }
}
