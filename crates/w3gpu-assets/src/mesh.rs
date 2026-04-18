use w3gpu_math::{Aabb, BoundingSphere, Vec3};

use crate::vertex::Vertex;

pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub bounding_sphere: BoundingSphere,
    pub aabb: Aabb,
}

impl Mesh {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        let positions: Vec<Vec3> = vertices.iter().map(|v| Vec3::from(v.position)).collect();
        let aabb = Aabb::from_points(&positions);
        let bounding_sphere = BoundingSphere::from_points(&positions);
        Self { vertices, indices, bounding_sphere, aabb }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(pos: [f32; 3]) -> Vertex { Vertex::new(pos, [0.0, 1.0, 0.0], [0.0, 0.0]) }

    #[test]
    fn aabb_computed_from_vertices() {
        let m = Mesh::new(vec![v([-1.0, 0.0, 0.5]), v([1.0, 2.0, -1.0]), v([0.0, -1.0, 0.0])], vec![0,1,2]);
        assert_eq!(m.aabb.min, Vec3::new(-1.0, -1.0, -1.0));
        assert_eq!(m.aabb.max, Vec3::new(1.0, 2.0, 0.5));
    }

    #[test]
    fn bounding_sphere_centroid_and_radius() {
        let m = Mesh::new(vec![v([-1.0, 0.0, 0.0]), v([1.0, 0.0, 0.0])], vec![0, 1]);
        assert!((m.bounding_sphere.center.x).abs() < 1e-5);
        assert!((m.bounding_sphere.radius - 1.0).abs() < 1e-5);
    }

    #[test]
    fn single_vertex_zero_radius() {
        let m = Mesh::new(vec![v([3.0, 5.0, 1.0])], vec![0]);
        assert_eq!(m.bounding_sphere.radius, 0.0);
        assert_eq!(m.aabb.min, m.aabb.max);
    }

    #[test]
    fn empty_mesh_default_bounds() {
        let m = Mesh::new(vec![], vec![]);
        assert_eq!(m.bounding_sphere.radius, 0.0);
    }
}
