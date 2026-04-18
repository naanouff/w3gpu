/// Interleaved vertex format — 20 floats / 80 bytes.
/// Matches the w3dts standard mesh vertex layout exactly.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position:  [f32; 3], // location 0
    pub uv0:       [f32; 2], // location 1
    pub uv1:       [f32; 2], // location 2
    pub normal:    [f32; 3], // location 3
    pub tangent:   [f32; 3], // location 4
    pub bitangent: [f32; 3], // location 5
    pub color:     [f32; 4], // location 6
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
