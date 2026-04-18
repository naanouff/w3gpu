pub mod error;
pub mod frame_uniforms;
pub mod gpu_context;
pub mod render_command;

pub use error::EngineError;
pub use frame_uniforms::FrameUniforms;
pub use gpu_context::GpuContext;
pub use render_command::RenderCommand;

pub const TRIANGLE_WGSL: &str = include_str!("shaders/triangle.wgsl");
