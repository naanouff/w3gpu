use w3gpu_ecs::{
    components::{CameraComponent, CulledComponent, RenderableComponent, TransformComponent},
    Entity, World,
};
use w3gpu_math::{BoundingSphere, Frustum, Vec3};

pub fn frustum_culling_system(world: &mut World, _dt: f32, _t: f32) {
    let vp = world
        .query_entities::<CameraComponent>()
        .into_iter()
        .find_map(|e| {
            let cam = world.get_component::<CameraComponent>(e)?;
            if cam.is_active {
                Some(cam.view_projection_matrix)
            } else {
                None
            }
        });

    let frustum = match vp {
        Some(vp) => Frustum::from_view_projection(&vp),
        None => return,
    };

    let renderables: Vec<Entity> = world.query_entities::<RenderableComponent>();

    for entity in renderables {
        let center: Vec3 = world
            .get_component::<TransformComponent>(entity)
            .map(|t| {
                let col = t.world_matrix.w_axis;
                Vec3::new(col.x, col.y, col.z)
            })
            .unwrap_or(Vec3::ZERO);

        // Conservative radius — Phase 3 will use actual mesh bounds
        let sphere = BoundingSphere::new(center, 2.0);

        if frustum.cull_sphere(&sphere) {
            if !world.has_component::<CulledComponent>(entity) {
                world.add_component(entity, CulledComponent);
            }
        } else {
            world.remove_component::<CulledComponent>(entity);
        }
    }
}
