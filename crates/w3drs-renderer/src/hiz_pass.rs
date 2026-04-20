use std::mem::size_of;

use crate::{
    asset_registry::AssetRegistry,
    gpu_context::DEPTH_FORMAT,
    light_uniforms::LightUniforms,
    render_command::DrawEntity,
    vertex_layout::VERTEX_BUFFER_LAYOUT,
};

const HIZ_INIT_WGSL: &str = include_str!("shaders/hiz_init.wgsl");
const HIZ_DOWN_WGSL: &str = include_str!("shaders/hiz_down.wgsl");

/// Z-prepass (depth-only from camera) + Hi-Z mip-pyramid construction.
///
/// Resize-aware: call `resize()` whenever the surface changes, then
/// `rebuild_needed` returns the new `hiz_full_view` so callers can
/// rebuild their Hi-Z bind group (e.g. `CullPass`).
pub struct HizPass {
    // ── z-prepass pipeline (reuses shadow_depth.wgsl, no fragment) ──────────
    pub zprepass_pipeline: wgpu::RenderPipeline,
    /// Uniform buffer holding the camera view_proj (LightUniforms layout).
    pub camera_buf: wgpu::Buffer,
    pub camera_bg: wgpu::BindGroup,

    // ── compute pipelines (static) ───────────────────────────────────────────
    init_pipeline:  wgpu::ComputePipeline,
    down_pipeline:  wgpu::ComputePipeline,
    init_bg_layout: wgpu::BindGroupLayout,
    down_bg_layout: wgpu::BindGroupLayout,

    // ── resolution-dependent resources ───────────────────────────────────────
    /// Depth-only render target for the z-prepass (screen resolution).
    pub zprepass_depth_view: wgpu::TextureView,
    /// Full mip-chain view of the Hi-Z R32Float texture — bind to cull pass.
    pub hiz_full_view: wgpu::TextureView,
    pub mip_count: u32,
    pub width:     u32,
    pub height:    u32,

    // internal per-mip compute bind groups (rebuilt on resize)
    init_bg:  wgpu::BindGroup,
    down_bgs: Vec<wgpu::BindGroup>,
}

impl HizPass {
    pub fn new(
        device: &wgpu::Device,
        instance_bg_layout: &wgpu::BindGroupLayout,
        width: u32,
        height: u32,
    ) -> Self {
        // ── camera uniform buffer (same layout as LightUniforms) ─────────────
        let camera_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("zprepass camera buf"),
            size: size_of::<LightUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bg_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("zprepass camera bg layout"),
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
        let camera_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("zprepass camera bg"),
            layout: &camera_bg_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buf.as_entire_binding(),
            }],
        });

        // ── z-prepass pipeline (shadow_depth.wgsl, depth-only) ───────────────
        let zp_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("zprepass shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/shadow_depth.wgsl").into(),
            ),
        });
        let zp_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("zprepass pipeline layout"),
            bind_group_layouts: &[&camera_bg_layout, instance_bg_layout],
            push_constant_ranges: &[],
        });
        let zprepass_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("zprepass pipeline"),
            layout: Some(&zp_layout),
            vertex: wgpu::VertexState {
                module: &zp_shader,
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
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // ── Hi-Z init pipeline (depth32 → r32float mip 0) ───────────────────
        let init_bg_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hiz init bg layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let init_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hiz init shader"),
            source: wgpu::ShaderSource::Wgsl(HIZ_INIT_WGSL.into()),
        });
        let init_pl_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&init_bg_layout],
            push_constant_ranges: &[],
        });
        let init_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("hiz init pipeline"),
            layout: Some(&init_pl_layout),
            module: &init_shader,
            entry_point: Some("cs_main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // ── Hi-Z down pipeline (r32float mip N → N+1) ───────────────────────
        let down_bg_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hiz down bg layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let down_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hiz down shader"),
            source: wgpu::ShaderSource::Wgsl(HIZ_DOWN_WGSL.into()),
        });
        let down_pl_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&down_bg_layout],
            push_constant_ranges: &[],
        });
        let down_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("hiz down pipeline"),
            layout: Some(&down_pl_layout),
            module: &down_shader,
            entry_point: Some("cs_main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let (zprepass_depth_view, hiz_full_view, mip_count, init_bg, down_bgs) =
            Self::build_resolution_resources(
                device, width, height, &init_bg_layout, &down_bg_layout,
            );

        Self {
            zprepass_pipeline,
            camera_buf,
            camera_bg,
            init_pipeline,
            down_pipeline,
            init_bg_layout,
            down_bg_layout,
            zprepass_depth_view,
            hiz_full_view,
            mip_count,
            width,
            height,
            init_bg,
            down_bgs,
        }
    }

    /// Recreate resolution-dependent resources on window resize.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == self.width && height == self.height { return; }
        self.width  = width;
        self.height = height;
        let (depth_view, hiz_view, mip_count, init_bg, down_bgs) =
            Self::build_resolution_resources(
                device, width, height, &self.init_bg_layout, &self.down_bg_layout,
            );
        self.zprepass_depth_view = depth_view;
        self.hiz_full_view       = hiz_view;
        self.mip_count           = mip_count;
        self.init_bg             = init_bg;
        self.down_bgs            = down_bgs;
    }

    /// Upload camera view_proj into the z-prepass uniform buffer.
    pub fn update_camera(&self, queue: &wgpu::Queue, view_proj: [[f32; 4]; 4]) {
        let uniforms = LightUniforms { view_proj, shadow_bias: 0.0, _pad: [0.0; 3] };
        queue.write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&uniforms));
    }

    /// Encode the z-prepass render pass and all Hi-Z compute passes.
    pub fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        instance_bind_group: &wgpu::BindGroup,
        registry: &AssetRegistry,
        sorted_entities: &[DrawEntity],
    ) {
        // ── z-prepass ─────────────────────────────────────────────────────────
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("z-prepass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.zprepass_depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            rp.set_pipeline(&self.zprepass_pipeline);
            rp.set_bind_group(0, &self.camera_bg, &[]);
            rp.set_bind_group(1, instance_bind_group, &[]);
            for (idx, entity) in sorted_entities.iter().enumerate() {
                let Some(mesh) = registry.get_mesh(entity.mesh_id) else { continue };
                rp.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                rp.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                let i = idx as u32;
                rp.draw_indexed(0..mesh.index_count, 0, i..i + 1);
            }
        }

        // ── Hi-Z init (depth32 → r32float mip 0) ─────────────────────────────
        {
            let mut cp = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("hiz init"),
                timestamp_writes: None,
            });
            cp.set_pipeline(&self.init_pipeline);
            cp.set_bind_group(0, &self.init_bg, &[]);
            cp.dispatch_workgroups(div_ceil(self.width, 8), div_ceil(self.height, 8), 1);
        }

        // ── Hi-Z downsample chain ─────────────────────────────────────────────
        let mut w = self.width;
        let mut h = self.height;
        for bg in &self.down_bgs {
            w = (w / 2).max(1);
            h = (h / 2).max(1);
            let mut cp = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("hiz down"),
                timestamp_writes: None,
            });
            cp.set_pipeline(&self.down_pipeline);
            cp.set_bind_group(0, bg, &[]);
            cp.dispatch_workgroups(div_ceil(w, 8), div_ceil(h, 8), 1);
        }
    }

    // ── private ──────────────────────────────────────────────────────────────

    fn build_resolution_resources(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        init_bg_layout: &wgpu::BindGroupLayout,
        down_bg_layout: &wgpu::BindGroupLayout,
    ) -> (wgpu::TextureView, wgpu::TextureView, u32, wgpu::BindGroup, Vec<wgpu::BindGroup>) {
        // Z-prepass depth texture (screen-sized)
        let zp_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("zprepass depth tex"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let zp_view = zp_tex.create_view(&Default::default());

        // Hi-Z mip count: ceil(log2(max(w,h))) + 1
        let mip_count = (u32::BITS - width.max(height).leading_zeros()).max(1);

        // Hi-Z R32Float texture (screen-sized, mip chain)
        let hiz_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hi-z tex"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: mip_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let hiz_full_view = hiz_tex.create_view(&Default::default());

        // One view per mip level (for compute read/write)
        let mip_views: Vec<wgpu::TextureView> = (0..mip_count)
            .map(|m| hiz_tex.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level:  m,
                mip_level_count: Some(1),
                ..Default::default()
            }))
            .collect();

        // Init bind group: zp_depth (binding 0) → hiz mip 0 (binding 1)
        let init_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hiz init bg"),
            layout: init_bg_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&zp_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&mip_views[0]),
                },
            ],
        });

        // Down bind groups: mip[i] (src) → mip[i+1] (dst)
        let down_bgs: Vec<wgpu::BindGroup> = (0..mip_count as usize - 1)
            .map(|i| device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("hiz down bg"),
                layout: down_bg_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&mip_views[i]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&mip_views[i + 1]),
                    },
                ],
            }))
            .collect();

        (zp_view, hiz_full_view, mip_count, init_bg, down_bgs)
    }
}

#[inline]
fn div_ceil(a: u32, b: u32) -> u32 { (a + b - 1) / b }
