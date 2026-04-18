/// A single draw call produced by the batching system.
pub struct RenderCommand {
    pub mesh_id: u32,
    pub material_id: u32,
    pub world_matrix: [[f32; 4]; 4],
    pub cast_shadow: bool,
}
