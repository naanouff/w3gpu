use crate::{
    frame_uniforms::SHADOW_CASCADE_COUNT, light_uniforms::LightUniforms,
    vertex_layout::VERTEX_BUFFER_LAYOUT,
};
use std::mem::size_of;

pub const SHADOW_SIZE: u32 = 2048;
const SHADOW_WGSL: &str = include_str!("shaders/shadow_depth.wgsl");

/// GPU resources for one directional-light shadow pass.
pub struct ShadowPass {
    /// Depth-only pipeline rendering scene from the light POV.
    pub depth_pipeline: wgpu::RenderPipeline,
    pub shadow_texture: wgpu::Texture,
    /// 2D-array view sampled in PBR (`texture_depth_2d_array`).
    pub shadow_array_view: wgpu::TextureView,
    /// Per-cascade single-layer depth views used as depth attachments.
    pub cascade_views: [wgpu::TextureView; SHADOW_CASCADE_COUNT],
    /// Comparison sampler for PCF — exposed so the engine can build group 3.
    pub comparison_sampler: wgpu::Sampler,
    /// One uniform buffer per cascade so each render pass sees its own matrix.
    pub light_uniform_buffers: [wgpu::Buffer; SHADOW_CASCADE_COUNT],
    /// One bind group per cascade referencing the corresponding buffer.
    pub shadow_light_bind_groups: [wgpu::BindGroup; SHADOW_CASCADE_COUNT],
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
                depth_or_array_layers: SHADOW_CASCADE_COUNT as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_array_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("shadow map array view"),
            format: Some(wgpu::TextureFormat::Depth32Float),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            usage: Some(
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            ),
            aspect: wgpu::TextureAspect::DepthOnly,
            base_mip_level: 0,
            mip_level_count: Some(1),
            base_array_layer: 0,
            array_layer_count: Some(SHADOW_CASCADE_COUNT as u32),
        });
        let cascade_views = std::array::from_fn(|i| {
            shadow_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("shadow map cascade view"),
                format: Some(wgpu::TextureFormat::Depth32Float),
                dimension: Some(wgpu::TextureViewDimension::D2),
                usage: Some(
                    wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                ),
                aspect: wgpu::TextureAspect::DepthOnly,
                base_mip_level: 0,
                mip_level_count: Some(1),
                base_array_layer: i as u32,
                array_layer_count: Some(1),
            })
        });

        // ── per-cascade light uniform buffers ──────────────────────────────────
        let shadow_light_bg_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("shadow light bg layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(size_of::<LightUniforms>() as u64),
                    },
                    count: None,
                }],
            });
        let light_uniform_buffers: [wgpu::Buffer; SHADOW_CASCADE_COUNT] =
            std::array::from_fn(|i| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("light uniforms cascade {i}")),
                    size: size_of::<LightUniforms>() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            });
        let shadow_light_bind_groups: [wgpu::BindGroup; SHADOW_CASCADE_COUNT] =
            std::array::from_fn(|i| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&format!("shadow light bind group cascade {i}")),
                    layout: &shadow_light_bg_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: light_uniform_buffers[i].as_entire_binding(),
                    }],
                })
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow depth pipeline layout"),
            bind_group_layouts: &[&shadow_light_bg_layout, instance_bg_layout],
            push_constant_ranges: &[],
        });

        let depth_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
            shadow_array_view,
            cascade_views,
            comparison_sampler,
            light_uniform_buffers,
            shadow_light_bind_groups,
        }
    }

    /// Write all cascade light uniforms in one go — safe with `queue.write_buffer`
    /// because each cascade targets a separate GPU buffer.
    pub fn update_cascade_lights(
        &self,
        queue: &wgpu::Queue,
        cascades: &[LightUniforms; SHADOW_CASCADE_COUNT],
    ) {
        for (i, u) in cascades.iter().enumerate() {
            queue.write_buffer(&self.light_uniform_buffers[i], 0, bytemuck::bytes_of(u));
        }
    }

    /// Backward-compat: writes to cascade 0 only (used by hdr-ibl-skybox).
    pub fn update_light(&self, queue: &wgpu::Queue, uniforms: &LightUniforms) {
        queue.write_buffer(
            &self.light_uniform_buffers[0],
            0,
            bytemuck::bytes_of(uniforms),
        );
    }

    #[inline]
    pub fn cascade_view(&self, cascade_idx: usize) -> &wgpu::TextureView {
        &self.cascade_views[cascade_idx]
    }
}
