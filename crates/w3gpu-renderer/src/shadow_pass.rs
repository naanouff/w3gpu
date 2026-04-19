use std::mem::size_of;
use crate::{light_uniforms::LightUniforms, vertex_layout::VERTEX_BUFFER_LAYOUT};

pub const SHADOW_SIZE: u32 = 2048;
const SHADOW_WGSL: &str = include_str!("shaders/shadow_depth.wgsl");

/// GPU resources for one directional-light shadow pass.
pub struct ShadowPass {
    /// Depth-only pipeline rendering scene from the light POV.
    pub depth_pipeline: wgpu::RenderPipeline,
    pub shadow_texture: wgpu::Texture,
    /// View into `shadow_texture` — used as depth attachment in shadow pass
    /// and as `texture_depth_2d` in the combined IBL+shadow bind group (group 3).
    pub shadow_view: wgpu::TextureView,
    /// Comparison sampler for PCF — exposed so the engine can build group 3.
    pub comparison_sampler: wgpu::Sampler,
    pub light_uniform_buffer: wgpu::Buffer,
    /// Bind group for group(0) in the shadow depth pass (LightUniforms, VERTEX).
    pub shadow_light_bind_group: wgpu::BindGroup,
}

impl ShadowPass {
    /// `instance_bg_layout` — storage-buffer instance layout from `RenderState`.
    pub fn new(device: &wgpu::Device, instance_bg_layout: &wgpu::BindGroupLayout) -> Self {
        // ── shadow depth texture ─────────────────────────────────────────────
        let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow map"),
            size: wgpu::Extent3d {
                width: SHADOW_SIZE,
                height: SHADOW_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow_texture.create_view(&Default::default());

        // ── light uniform buffer ─────────────────────────────────────────────
        let light_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("light uniforms"),
            size: size_of::<LightUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ── group 0 layout for shadow depth pass (LightUniforms, VERTEX only) ─
        let shadow_light_bg_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("shadow light bg layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            size_of::<LightUniforms>() as u64,
                        ),
                    },
                    count: None,
                }],
            });

        let shadow_light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow light bind group"),
            layout: &shadow_light_bg_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_uniform_buffer.as_entire_binding(),
            }],
        });

        // ── comparison sampler (PCF) — stored so engine can build group 3 ────
        let comparison_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow comparison sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        // ── shadow depth pipeline ─────────────────────────────────────────────
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shadow depth shader"),
            source: wgpu::ShaderSource::Wgsl(SHADOW_WGSL.into()),
        });

        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("shadow depth pipeline layout"),
                bind_group_layouts: &[&shadow_light_bg_layout, instance_bg_layout],
                push_constant_ranges: &[],
            });

        let depth_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("shadow depth pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[VERTEX_BUFFER_LAYOUT],
                    compilation_options: Default::default(),
                },
                fragment: None,
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    // slope-scaled bias prevents self-shadowing artifacts
                    bias: wgpu::DepthBiasState {
                        constant: 1,
                        slope_scale: 1.0,
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        Self {
            depth_pipeline,
            shadow_texture,
            shadow_view,
            comparison_sampler,
            light_uniform_buffer,
            shadow_light_bind_group,
        }
    }

    pub fn update_light(&self, queue: &wgpu::Queue, uniforms: &LightUniforms) {
        queue.write_buffer(
            &self.light_uniform_buffer,
            0,
            bytemuck::bytes_of(uniforms),
        );
    }
}
