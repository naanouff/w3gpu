use glam::{Mat4, Quat, Vec3};

/// Decompose a Mat4 into (translation, rotation, scale).
pub fn decompose_trs(m: &Mat4) -> (Vec3, Quat, Vec3) {
    let (scale, rotation, translation) = m.to_scale_rotation_translation();
    (translation, rotation, scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Quat, Vec3};

    #[test]
    fn decompose_identity() {
        let (t, r, s) = decompose_trs(&Mat4::IDENTITY);
        assert!(t.length() < 1e-5);
        assert!((r.w - 1.0).abs() < 1e-5);
        assert!((s - Vec3::ONE).length() < 1e-5);
    }

    #[test]
    fn decompose_translation() {
        let m = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
        let (t, _, _) = decompose_trs(&m);
        assert!((t - Vec3::new(1.0, 2.0, 3.0)).length() < 1e-5);
    }

    #[test]
    fn decompose_scale() {
        let m = Mat4::from_scale(Vec3::new(2.0, 3.0, 4.0));
        let (_, _, s) = decompose_trs(&m);
        assert!((s - Vec3::new(2.0, 3.0, 4.0)).length() < 1e-4);
    }

    #[test]
    fn decompose_combined() {
        let m = Mat4::from_scale_rotation_translation(
            Vec3::new(2.0, 2.0, 2.0),
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            Vec3::new(5.0, 0.0, 0.0),
        );
        let (t, _, s) = decompose_trs(&m);
        assert!((t - Vec3::new(5.0, 0.0, 0.0)).length() < 1e-4);
        assert!((s - Vec3::splat(2.0)).length() < 1e-4);
    }
}
