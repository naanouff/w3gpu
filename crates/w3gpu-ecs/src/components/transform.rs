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
