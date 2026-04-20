use glam::{Mat4, Quat, Vec3};

#[derive(Clone, Debug)]
pub struct TransformComponent {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub local_matrix: Mat4,
    pub world_matrix: Mat4,
    pub dirty: bool,
}

impl TransformComponent {
    pub fn new(position: Vec3, rotation: Quat, scale: Vec3) -> Self {
        let local_matrix = Mat4::from_scale_rotation_translation(scale, rotation, position);
        Self {
            position,
            rotation,
            scale,
            local_matrix,
            world_matrix: local_matrix,
            dirty: true,
        }
    }

    pub fn from_position(position: Vec3) -> Self {
        Self::new(position, Quat::IDENTITY, Vec3::ONE)
    }

    pub fn update_local_matrix(&mut self) {
        self.local_matrix = Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position);
        self.dirty = true;
    }
}

impl Default for TransformComponent {
    fn default() -> Self {
        Self::new(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_identity_transform() {
        let t = TransformComponent::default();
        assert_eq!(t.position, Vec3::ZERO);
        assert_eq!(t.rotation, Quat::IDENTITY);
        assert_eq!(t.scale, Vec3::ONE);
        assert!(t.dirty);
    }

    #[test]
    fn from_position_sets_position() {
        let t = TransformComponent::from_position(Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(t.position, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(t.scale, Vec3::ONE);
        assert_eq!(t.rotation, Quat::IDENTITY);
    }

    #[test]
    fn new_computes_local_matrix() {
        let pos = Vec3::new(1.0, 0.0, 0.0);
        let t = TransformComponent::new(pos, Quat::IDENTITY, Vec3::ONE);
        let expected = Mat4::from_translation(pos);
        assert!((t.local_matrix.w_axis - expected.w_axis).length() < 1e-5);
    }

    #[test]
    fn update_local_matrix_recomputes() {
        let mut t = TransformComponent::default();
        t.position = Vec3::new(5.0, 0.0, 0.0);
        t.update_local_matrix();
        // world_matrix should have translation 5 in x after update
        assert!((t.local_matrix.w_axis.x - 5.0).abs() < 1e-5);
    }

    #[test]
    fn dirty_flag_set_after_update() {
        let mut t = TransformComponent::default();
        t.dirty = false;
        t.update_local_matrix();
        assert!(t.dirty);
    }
}
