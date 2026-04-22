use std::collections::VecDeque;

use glam::Mat4;
use w3drs_ecs::{
    components::{HierarchyComponent, TransformComponent},
    Entity, World,
};

/// World-matrix update — two passes for maximum parallelism.
///
/// Pass 1 (parallel on native): entities in archetypes WITHOUT HierarchyComponent.
///   world_matrix = local_matrix  (independent per entity → Rayon par_iter_mut)
///
/// Pass 2 (serial BFS): entities WITH HierarchyComponent that have a parent.
///   world_matrix = parent.world_matrix * local_matrix
///   (must run top-down, inherently sequential)
///
/// For typical scenes (100k flat objects, no hierarchy) pass 2 is a no-op,
/// and pass 1 saturates all CPU cores.
pub fn transform_system(world: &mut World, _dt: f32, _t: f32) {
    // ── Pass 1: flat entities (no HierarchyComponent) ─────────────────────
    world.for_each_without_mut::<TransformComponent, HierarchyComponent, _>(|t| {
        if t.dirty {
            t.world_matrix = t.local_matrix;
            t.dirty = false;
        }
    });

    // ── Pass 2: hierarchical entities — BFS top-down ──────────────────────
    let hierarchical: Vec<Entity> = world.query_entities::<HierarchyComponent>();
    if hierarchical.is_empty() {
        return;
    }

    // Roots = have HierarchyComponent with no parent.
    let roots: Vec<Entity> = hierarchical
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

        let parent_world: Mat4 = world
            .get_component::<HierarchyComponent>(entity)
            .and_then(|h| h.parent)
            .and_then(|p| world.get_component::<TransformComponent>(p))
            .map(|t| t.world_matrix)
            .unwrap_or(Mat4::IDENTITY);

        if let Some(t) = world.get_component_mut::<TransformComponent>(entity) {
            t.world_matrix = parent_world * local;
            t.dirty = false;
        }

        let children: Vec<Entity> = world
            .get_component::<HierarchyComponent>(entity)
            .map(|h| h.children.clone())
            .unwrap_or_default();
        queue.extend(children);
    }
}
