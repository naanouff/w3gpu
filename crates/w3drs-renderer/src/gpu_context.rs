use wgpu::{Device, Queue, Surface, SurfaceConfiguration};

use crate::{error::EngineError, hdr_target::pick_hdr_main_pass_msaa};

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub struct GpuContext {
    pub device: Device,
    pub queue: Queue,
    pub surface: Surface<'static>,
    pub surface_config: SurfaceConfiguration,
    pub surface_format: wgpu::TextureFormat,
    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
    /// Facteur MSAA pour le pass PBR HDR principal (1 si non supporté).
    pub main_pass_msaa: u32,
}

impl GpuContext {
    pub async fn new(
        instance: &wgpu::Instance,
        surface: Surface<'static>,
        width: u32,
        height: u32,
    ) -> Result<Self, EngineError> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or(EngineError::NoAdapter)?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("w3drs device"),
                    required_features: wgpu::Features::empty(),
                    // Use WebGPU-tier limits (not WebGL2 downlevel) — storage buffers
                    // in vertex shaders are required for instanced draw indirect (Phase 4).
                    required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let main_pass_msaa = pick_hdr_main_pass_msaa(&adapter);
        let (depth_texture, depth_view) =
            create_depth_texture(&device, width, height, main_pass_msaa);

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            surface_format,
            depth_texture,
            depth_view,
            main_pass_msaa,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        (self.depth_texture, self.depth_view) =
            create_depth_texture(&self.device, width, height, self.main_pass_msaa);
    }
}

pub fn create_depth_texture(
    device: &Device,
    width: u32,
    height: u32,
    sample_count: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let sample_count = sample_count.max(1);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
