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
