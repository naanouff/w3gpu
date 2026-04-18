use crate::entity::Entity;

#[derive(Clone, Debug, Default)]
pub struct HierarchyComponent {
    pub parent: Option<Entity>,
    pub children: Vec<Entity>,
}
