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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults_visible_and_shadow() {
        let r = RenderableComponent::new(1, 2);
        assert_eq!(r.mesh_id, 1);
        assert_eq!(r.material_id, 2);
        assert!(r.visible);
        assert!(r.cast_shadow);
        assert!(r.receive_shadow);
    }
}
