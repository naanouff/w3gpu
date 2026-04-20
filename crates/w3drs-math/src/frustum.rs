use glam::{Mat4, Vec4};

use crate::BoundingSphere;

/// 6 planes extracted from a view-projection matrix.
/// Each plane is (nx, ny, nz, d) where the positive half-space is inside the frustum.
#[derive(Clone, Copy, Debug)]
pub struct Frustum {
    planes: [Vec4; 6],
}

impl Frustum {
    /// Extract frustum planes from a combined view-projection matrix.
    /// Uses the Gribb/Hartmann method — same as w3dts frustum-culling.system.ts.
    pub fn from_view_projection(vp: &Mat4) -> Self {
        let m = vp.to_cols_array_2d();
        // rows indexed as m[col][row] with glam column-major
        let row = |r: usize| Vec4::new(m[0][r], m[1][r], m[2][r], m[3][r]);
        let r0 = row(0);
        let r1 = row(1);
        let r2 = row(2);
        let r3 = row(3);

        let planes = [
            (r3 + r0).normalize_or_zero(), // left
            (r3 - r0).normalize_or_zero(), // right
            (r3 + r1).normalize_or_zero(), // bottom
            (r3 - r1).normalize_or_zero(), // top
            (r3 + r2).normalize_or_zero(), // near
            (r3 - r2).normalize_or_zero(), // far
        ];
        Self { planes }
    }

    /// Returns true if the sphere is fully outside the frustum (should be culled).
    pub fn cull_sphere(&self, sphere: &BoundingSphere) -> bool {
        let center = sphere.center.extend(1.0);
        for plane in &self.planes {
            if plane.dot(center) < -sphere.radius {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Mat4, Vec3};

    fn test_frustum() -> Frustum {
        let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 0.1, 100.0);
        Frustum::from_view_projection(&proj)
    }

    #[test]
    fn sphere_in_front_not_culled() {
        let f = test_frustum();
        let s = BoundingSphere::new(Vec3::new(0.0, 0.0, -5.0), 0.5);
        assert!(!f.cull_sphere(&s));
    }

    #[test]
    fn sphere_far_behind_culled() {
        let f = test_frustum();
        let s = BoundingSphere::new(Vec3::new(0.0, 0.0, 200.0), 0.5);
        assert!(f.cull_sphere(&s));
    }

    #[test]
    fn sphere_far_right_culled() {
        let f = test_frustum();
        let s = BoundingSphere::new(Vec3::new(1000.0, 0.0, -5.0), 0.1);
        assert!(f.cull_sphere(&s));
    }

    #[test]
    fn large_sphere_enclosing_frustum_not_culled() {
        let f = test_frustum();
        let s = BoundingSphere::new(Vec3::new(0.0, 0.0, -50.0), 500.0);
        assert!(!f.cull_sphere(&s));
    }

    #[test]
    fn sphere_behind_near_plane_culled() {
        let f = test_frustum();
        // positive Z = behind camera in RH
        let s = BoundingSphere::new(Vec3::new(0.0, 0.0, 1.0), 0.01);
        assert!(f.cull_sphere(&s));
    }
}
