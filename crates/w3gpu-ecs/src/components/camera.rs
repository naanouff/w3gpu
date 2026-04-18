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
