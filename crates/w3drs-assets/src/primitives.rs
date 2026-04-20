use crate::mesh::Mesh;
use crate::vertex::Vertex;

pub fn triangle() -> Mesh {
    let vertices = vec![
        Vertex::new([ 0.0,  0.5, 0.0], [0.0, 0.0, 1.0], [0.5, 0.0]),
        Vertex::new([-0.5, -0.5, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0]),
        Vertex::new([ 0.5, -0.5, 0.0], [0.0, 0.0, 1.0], [1.0, 1.0]),
    ];
    Mesh::new(vertices, vec![0, 1, 2])
}

pub fn cube() -> Mesh {
    #[rustfmt::skip]
    let faces: &[([f32; 3], [f32; 3])] = &[
        // position offset, normal
        ([ 0.0,  0.0,  0.5], [0.0,  0.0,  1.0]), // front
        ([ 0.0,  0.0, -0.5], [0.0,  0.0, -1.0]), // back
        ([-0.5,  0.0,  0.0], [-1.0, 0.0,  0.0]), // left
        ([ 0.5,  0.0,  0.0], [ 1.0, 0.0,  0.0]), // right
        ([ 0.0,  0.5,  0.0], [0.0,  1.0,  0.0]), // top
        ([ 0.0, -0.5,  0.0], [0.0, -1.0,  0.0]), // bottom
    ];

    let local_verts: &[[f32; 3]; 4] = &[
        [-0.5, -0.5, 0.0],
        [ 0.5, -0.5, 0.0],
        [ 0.5,  0.5, 0.0],
        [-0.5,  0.5, 0.0],
    ];
    let uvs: &[[f32; 2]; 4] = &[[0.0,1.0],[1.0,1.0],[1.0,0.0],[0.0,0.0]];

    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);

    for (face_idx, (offset, normal)) in faces.iter().enumerate() {
        let base = (face_idx * 4) as u32;

        // Build a rotation that takes +Z face to target normal
        let n = glam::Vec3::from(*normal);
        let rot = glam::Quat::from_rotation_arc(glam::Vec3::Z, n);

        for (i, lv) in local_verts.iter().enumerate() {
            let lp = glam::Vec3::from(*lv);
            let rotated = rot * lp;
            let pos = [
                rotated.x + offset[0],
                rotated.y + offset[1],
                rotated.z + offset[2],
            ];
            vertices.push(Vertex::new(pos, *normal, uvs[i]));
        }
        indices.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
    }

    Mesh::new(vertices, indices)
}

/// UV sphere — radius `r`, `stacks` latitude bands, `sectors` longitude segments.
/// Tangents and bitangents are analytically correct for PBR normal mapping.
pub fn uv_sphere(radius: f32, stacks: u32, sectors: u32) -> Mesh {
    let stacks  = stacks.max(2);
    let sectors = sectors.max(3);
    let mut vertices = Vec::with_capacity(((stacks + 1) * (sectors + 1)) as usize);
    let mut indices  = Vec::with_capacity((stacks * sectors * 6) as usize);

    for i in 0..=stacks {
        let phi     = std::f32::consts::PI * i as f32 / stacks as f32;
        let cos_phi = phi.cos();
        let sin_phi = phi.sin();
        let y       = radius * cos_phi;
        let ring_r  = radius * sin_phi;

        for j in 0..=sectors {
            let theta     = std::f32::consts::TAU * j as f32 / sectors as f32;
            let cos_theta = theta.cos();
            let sin_theta = theta.sin();
            let x = ring_r * cos_theta;
            let z = ring_r * sin_theta;
            let normal  = [sin_phi * cos_theta, cos_phi, sin_phi * sin_theta];
            let uv      = [j as f32 / sectors as f32, i as f32 / stacks as f32];
            let mut v   = Vertex::new([x, y, z], normal, uv);
            v.tangent   = [-sin_theta, 0.0, cos_theta];
            v.bitangent = [cos_phi * cos_theta, -sin_phi, cos_phi * sin_theta];
            vertices.push(v);
        }
    }

    let sw = sectors + 1;
    for i in 0..stacks {
        for j in 0..sectors {
            let k0 = i * sw + j;
            let k1 = (i + 1) * sw + j;
            indices.extend_from_slice(&[k0, k0 + 1, k1, k1, k0 + 1, k1 + 1]);
        }
    }

    Mesh::new(vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangle_has_3_vertices_and_3_indices() {
        let m = triangle();
        assert_eq!(m.vertices.len(), 3);
        assert_eq!(m.indices.len(), 3);
    }

    #[test]
    fn cube_has_24_vertices_and_36_indices() {
        let m = cube();
        assert_eq!(m.vertices.len(), 24);
        assert_eq!(m.indices.len(), 36);
    }

    #[test]
    fn cube_positions_within_unit_cube() {
        for v in cube().vertices {
            assert!(v.position[0].abs() <= 0.5 + 1e-5);
            assert!(v.position[1].abs() <= 0.5 + 1e-5);
            assert!(v.position[2].abs() <= 0.5 + 1e-5);
        }
    }

    #[test]
    fn cube_normals_are_unit_length() {
        for v in cube().vertices {
            let n = v.normal;
            let len = (n[0]*n[0] + n[1]*n[1] + n[2]*n[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-5, "normal not unit: {}", len);
        }
    }

    #[test]
    fn cube_indices_in_range() {
        let m = cube();
        let max_idx = m.vertices.len() as u32;
        for &i in &m.indices {
            assert!(i < max_idx, "index {} out of range", i);
        }
    }

    #[test]
    fn triangle_indices_in_range() {
        let m = triangle();
        for &i in &m.indices {
            assert!(i < m.vertices.len() as u32);
        }
    }
}
