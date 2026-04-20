use crate::entity::Entity;

#[derive(Clone, Debug, Default)]
pub struct HierarchyComponent {
    pub parent: Option<Entity>,
    pub children: Vec<Entity>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_no_parent_or_children() {
        let h = HierarchyComponent::default();
        assert!(h.parent.is_none());
        assert!(h.children.is_empty());
    }
}
