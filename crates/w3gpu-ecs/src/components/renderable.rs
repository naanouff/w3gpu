#[derive(Clone, Debug)]
pub struct RenderableComponent {
    pub mesh_id: u32,
    pub material_id: u32,
    pub visible: bool,
    pub cast_shadow: bool,
    pub receive_shadow: bool,
}

impl RenderableComponent {
    pub fn new(mesh_id: u32, material_id: u32) -> Self {
        Self {
            mesh_id,
            material_id,
            visible: true,
            cast_shadow: true,
            receive_shadow: true,
        }
    }
}
