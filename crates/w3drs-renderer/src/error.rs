use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("No suitable GPU adapter found")]
    NoAdapter,
    #[error("GPU device request failed: {0}")]
    DeviceRequest(#[from] wgpu::RequestDeviceError),
    #[error("Surface configuration error: {0}")]
    SurfaceError(String),
    #[error("Shader compilation error: {0}")]
    ShaderError(String),
}
