/// Interleaved vertex format — 20 floats / 80 bytes.
/// Matches the w3dts standard mesh vertex layout exactly.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],  // location 0
    pub uv0: [f32; 2],       // location 1
    pub uv1: [f32; 2],       // location 2
    pub normal: [f32; 3],    // location 3
    pub tangent: [f32; 3],   // location 4
    pub bitangent: [f32; 3], // location 5
    pub color: [f32; 4],     // location 6
}

impl Vertex {
    pub const SIZE: usize = std::mem::size_of::<Self>();

    pub fn new(position: [f32; 3], normal: [f32; 3], uv0: [f32; 2]) -> Self {
        Self {
            position,
            uv0,
            uv1: uv0,
            normal,
            tangent: [1.0, 0.0, 0.0],
            bitangent: [0.0, 1.0, 0.0],
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_is_80_bytes() {
        assert_eq!(Vertex::SIZE, 80);
        assert_eq!(std::mem::size_of::<Vertex>(), 80);
    }

    #[test]
    fn new_sets_position_normal_uv() {
        let v = Vertex::new([1.0, 2.0, 3.0], [0.0, 1.0, 0.0], [0.5, 0.5]);
        assert_eq!(v.position, [1.0, 2.0, 3.0]);
        assert_eq!(v.normal, [0.0, 1.0, 0.0]);
        assert_eq!(v.uv0, [0.5, 0.5]);
        assert_eq!(v.uv1, [0.5, 0.5]); // uv1 mirrors uv0
    }

    #[test]
    fn new_default_color_is_white() {
        let v = Vertex::new([0.0; 3], [0.0, 1.0, 0.0], [0.0; 2]);
        assert_eq!(v.color, [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn pod_zeroable_safe() {
        // bytemuck guarantees: all-zeros is a valid bit pattern
        let _: Vertex = bytemuck::Zeroable::zeroed();
    }
}
