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
