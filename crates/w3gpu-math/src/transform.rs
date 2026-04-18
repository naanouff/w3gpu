use glam::{Mat4, Quat, Vec3};

/// Decompose a Mat4 into (translation, rotation, scale).
pub fn decompose_trs(m: &Mat4) -> (Vec3, Quat, Vec3) {
    let (scale, rotation, translation) = m.to_scale_rotation_translation();
    (translation, rotation, scale)
}
