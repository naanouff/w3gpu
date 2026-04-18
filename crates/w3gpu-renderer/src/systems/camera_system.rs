use glam::Mat4;
use w3gpu_ecs::{
    components::{CameraComponent, TransformComponent},
    Entity, World,
};

pub fn camera_system(world: &mut World, _dt: f32, _t: f32) {
    let cameras: Vec<Entity> = world.query_entities::<CameraComponent>();

    for entity in cameras {
        let world_matrix: Mat4 = world
            .get_component::<TransformComponent>(entity)
            .map(|t| t.world_matrix)
            .unwrap_or(Mat4::IDENTITY);

        let (fov, aspect, near, far) = match world.get_component::<CameraComponent>(entity) {
            Some(c) => (c.fov_y_radians, c.aspect, c.near, c.far),
            None => continue,
        };

        let view = world_matrix.inverse();
        let projection = Mat4::perspective_rh(fov, aspect, near, far);
        let view_projection = projection * view;

        if let Some(cam) = world.get_component_mut::<CameraComponent>(entity) {
            cam.view_matrix = view;
            cam.projection_matrix = projection;
            cam.view_projection_matrix = view_projection;
        }
    }
}
