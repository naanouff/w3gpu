pub mod camera;
pub mod dir_light;
pub mod hierarchy;
pub mod name;
pub mod point_light;
pub mod renderable;
pub mod spot_light;
pub mod transform;

pub use camera::CameraComponent;
pub use dir_light::DirectionalLightComponent;
pub use hierarchy::HierarchyComponent;
pub use name::NameComponent;
pub use point_light::PointLightComponent;
pub use renderable::RenderableComponent;
pub use spot_light::SpotLightComponent;
pub use transform::TransformComponent;

/// Tag component — zero-size, marks a frustum-culled entity.
#[derive(Clone, Copy, Default, Debug)]
pub struct CulledComponent;
