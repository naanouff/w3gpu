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
