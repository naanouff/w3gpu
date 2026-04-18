use std::collections::VecDeque;

use glam::Mat4;
use w3gpu_ecs::{
    components::{HierarchyComponent, TransformComponent},
    Entity, World,
};

/// Iterative world-matrix update — BFS from roots to children.
/// Avoids recursion (no stack overflow on deep glTF hierarchies).
pub fn transform_system(world: &mut World, _dt: f32, _t: f32) {
    let all: Vec<Entity> = world.query_entities::<TransformComponent>();

    // Roots = entities with no parent
    let roots: Vec<Entity> = all
        .iter()
        .copied()
        .filter(|&e| {
            world
                .get_component::<HierarchyComponent>(e)
                .map(|h| h.parent.is_none())
                .unwrap_or(true)
        })
        .collect();

    let mut queue: VecDeque<Entity> = VecDeque::from(roots);

    while let Some(entity) = queue.pop_front() {
        let local: Mat4 = world
            .get_component::<TransformComponent>(entity)
            .map(|t| t.local_matrix)
            .unwrap_or(Mat4::IDENTITY);

        let parent: Option<Entity> = world
            .get_component::<HierarchyComponent>(entity)
            .and_then(|h| h.parent);

        let parent_world: Mat4 = match parent {
            Some(p) => world
                .get_component::<TransformComponent>(p)
                .map(|t| t.world_matrix)
                .unwrap_or(Mat4::IDENTITY),
            None => Mat4::IDENTITY,
        };

        let world_matrix = parent_world * local;

        if let Some(t) = world.get_component_mut::<TransformComponent>(entity) {
            t.world_matrix = world_matrix;
            t.dirty = false;
        }

        let children: Vec<Entity> = world
            .get_component::<HierarchyComponent>(entity)
            .map(|h| h.children.clone())
            .unwrap_or_default();
        queue.extend(children);
    }
}
