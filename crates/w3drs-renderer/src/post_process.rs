use wgpu::util::DeviceExt;

const BLOOM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const BLUR_ITERS: u32 = 2;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BloomParams {
    pub threshold: f32,
    pub knee: f32,
    pub _pad0: f32,
    pub _pad1: f32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TonemapParams {
    pub exposure: f32,
    pub bloom_strength: f32,
    /// Bit **0** : si `1`, le fragment tonemap **saute le passage FXAA** (ACES + gamma seulement).
    pub flags: u32,
    pub _pad1: f32,
}

impl TonemapParams {
    pub const FLAG_SKIP_FXAA: u32 = 1;
}

struct BloomTextures {
    #[allow(dead_code)]
    bloom_a: wgpu::Texture,
    view_a: wgpu::TextureView,
    #[allow(dead_code)]
    bloom_b: wgpu::Texture,
    view_b: wgpu::TextureView,
}

impl BloomTextures {
    fn new(device: &wgpu::Device, hdr_w: u32, hdr_h: u32) -> Self {
        let w = (hdr_w / 2).max(1);
        let h = (hdr_h / 2).max(1);
        let mk = |label| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: BLOOM_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        };
        let bloom_a = mk("bloom_a");
        let view_a = bloom_a.create_view(&Default::default());
        let bloom_b = mk("bloom_b");
        let view_b = bloom_b.create_view(&Default::default());
        Self {
            bloom_a,
            view_a,
            bloom_b,
            view_b,
        }
    }
}

pub struct PostProcessPass {
    bloom_params_buf: wgpu::Buffer,
    tonemap_params_buf: wgpu::Buffer,
    sampler_linear: wgpu::Sampler,

    // pipelines
    prefilter_pipeline: wgpu::RenderPipeline,
    blur_h_pipeline: wgpu::RenderPipeline,
    blur_v_pipeline: wgpu::RenderPipeline,
    tonemap_pipeline: wgpu::RenderPipeline,

    // bind group layouts (cached for rebuild on resize)
    pf_bgl: wgpu::BindGroupLayout,
    blur_bgl: wgpu::BindGroupLayout,
    tonemap_bgl: wgpu::BindGroupLayout,

    // dynamic resources (rebuilt on resize)
    bloom: BloomTextures,
    pf_bg: wgpu::BindGroup,
    blur_h_bg: wgpu::BindGroup, // bloom_a → bloom_b
    blur_v_bg: wgpu::BindGroup, // bloom_b → bloom_a
    tonemap_bg: wgpu::BindGroup,

    #[allow(dead_code)]
    swapchain_format: wgpu::TextureFormat,
}

impl PostProcessPass {
    pub fn new(
        device: &wgpu::Device,
        hdr_view: &wgpu::TextureView,
        swapchain_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        bloom_params: BloomParams,
        tonemap_params: TonemapParams,
    ) -> Self {
        let bloom_params_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("bloom_params"),
            contents: bytemuck::bytes_of(&bloom_params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let tonemap_params_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tonemap_params"),
            contents: bytemuck::bytes_of(&tonemap_params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let sampler_linear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("pp_linear"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // ── Bind group layouts ─────────────────────────────────────────────────
        let tex_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let samp_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };
        let uniform_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };

        let pf_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pf_bgl"),
            entries: &[tex_entry(0), samp_entry(1), uniform_entry(2)],
        });
        let blur_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blur_bgl"),
            entries: &[tex_entry(0), samp_entry(1)],
        });
        let tonemap_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tonemap_bgl"),
            entries: &[tex_entry(0), tex_entry(1), samp_entry(2), uniform_entry(3)],
        });

        // ── Shaders ────────────────────────────────────────────────────────────
        let bloom_src = include_str!("shaders/bloom.wgsl");
        let tonemap_src = include_str!("shaders/tonemap.wgsl");

        let bloom_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bloom"),
            source: wgpu::ShaderSource::Wgsl(bloom_src.into()),
        });
        let tonemap_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tonemap"),
            source: wgpu::ShaderSource::Wgsl(tonemap_src.into()),
        });

        // ── Pipelines ─────────────────────────────────────────────────────────
        let mk_pipeline = |label: &str,
                           module: &wgpu::ShaderModule,
                           vs_entry: &str,
                           fs_entry: &str,
                           bgl: &wgpu::BindGroupLayout,
                           format: wgpu::TextureFormat| {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[bgl],
                push_constant_ranges: &[],
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: Some(vs_entry),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some(fs_entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
        };

        let prefilter_pipeline = mk_pipeline(
            "prefilter",
            &bloom_mod,
            "vs_fullscreen",
            "fs_prefilter",
            &pf_bgl,
            BLOOM_FORMAT,
        );
        let blur_h_pipeline = mk_pipeline(
            "blur_h",
            &bloom_mod,
            "vs_fullscreen",
            "fs_blur_h",
            &blur_bgl,
            BLOOM_FORMAT,
        );
        let blur_v_pipeline = mk_pipeline(
            "blur_v",
            &bloom_mod,
            "vs_fullscreen",
            "fs_blur_v",
            &blur_bgl,
            BLOOM_FORMAT,
        );
        let tonemap_pipeline = mk_pipeline(
            "tonemap",
            &tonemap_mod,
            "vs_fullscreen",
            "fs_tonemap",
            &tonemap_bgl,
            swapchain_format,
        );

        let bloom = BloomTextures::new(device, width, height);

        let (pf_bg, blur_h_bg, blur_v_bg, tonemap_bg) = Self::make_bind_groups(
            device,
            &pf_bgl,
            &blur_bgl,
            &tonemap_bgl,
            hdr_view,
            &bloom,
            &sampler_linear,
            &bloom_params_buf,
            &tonemap_params_buf,
        );

        Self {
            bloom_params_buf,
            tonemap_params_buf,
            sampler_linear,
            prefilter_pipeline,
            blur_h_pipeline,
            blur_v_pipeline,
            tonemap_pipeline,
            pf_bgl,
            blur_bgl,
            tonemap_bgl,
            bloom,
            pf_bg,
            blur_h_bg,
            blur_v_bg,
            tonemap_bg,
            swapchain_format,
        }
    }

    fn make_bind_groups(
        device: &wgpu::Device,
        pf_bgl: &wgpu::BindGroupLayout,
        blur_bgl: &wgpu::BindGroupLayout,
        tonemap_bgl: &wgpu::BindGroupLayout,
        hdr_view: &wgpu::TextureView,
        bloom: &BloomTextures,
        sampler: &wgpu::Sampler,
        bloom_params_buf: &wgpu::Buffer,
        tonemap_params_buf: &wgpu::Buffer,
    ) -> (
        wgpu::BindGroup,
        wgpu::BindGroup,
        wgpu::BindGroup,
        wgpu::BindGroup,
    ) {
        let pf_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pf_bg"),
            layout: pf_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(hdr_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: bloom_params_buf.as_entire_binding(),
                },
            ],
        });
        let blur_h_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blur_h_bg"),
            layout: blur_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&bloom.view_a),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        let blur_v_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blur_v_bg"),
            layout: blur_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&bloom.view_b),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        let tonemap_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tonemap_bg"),
            layout: tonemap_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(hdr_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&bloom.view_a),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: tonemap_params_buf.as_entire_binding(),
                },
            ],
        });
        (pf_bg, blur_h_bg, blur_v_bg, tonemap_bg)
    }

    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        hdr_view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        self.bloom = BloomTextures::new(device, width, height);
        let (pf_bg, blur_h_bg, blur_v_bg, tonemap_bg) = Self::make_bind_groups(
            device,
            &self.pf_bgl,
            &self.blur_bgl,
            &self.tonemap_bgl,
            hdr_view,
            &self.bloom,
            &self.sampler_linear,
            &self.bloom_params_buf,
            &self.tonemap_params_buf,
        );
        self.pf_bg = pf_bg;
        self.blur_h_bg = blur_h_bg;
        self.blur_v_bg = blur_v_bg;
        self.tonemap_bg = tonemap_bg;
    }

    pub fn update_bloom_params(&self, queue: &wgpu::Queue, p: BloomParams) {
        queue.write_buffer(&self.bloom_params_buf, 0, bytemuck::bytes_of(&p));
    }

    pub fn update_tonemap_params(&self, queue: &wgpu::Queue, p: TonemapParams) {
        queue.write_buffer(&self.tonemap_params_buf, 0, bytemuck::bytes_of(&p));
    }

    /// Tonemap (ACES + FXAA optionnel via [`TonemapParams::flags`]) + HDR → swapchain **sans**
    /// préfiltre bloom ni flous — la texture bloom est vidée au noir ; utiliser `bloom_strength: 0.0`
    /// pour un rendu neutre.
    pub fn encode_tonemap_only(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        swapchain_view: &wgpu::TextureView,
    ) {
        {
            let _rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bloom_stub_clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.bloom.view_a,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tonemap"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: swapchain_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&self.tonemap_pipeline);
            rp.set_bind_group(0, &self.tonemap_bg, &[]);
            rp.draw(0..3, 0..1);
        }
    }

    /// Run all post-process passes, writing final LDR output to `swapchain_view`.
    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder, swapchain_view: &wgpu::TextureView) {
        // Prefilter: HDR → bloom_a
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bloom_prefilter"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.bloom.view_a,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&self.prefilter_pipeline);
            rp.set_bind_group(0, &self.pf_bg, &[]);
            rp.draw(0..3, 0..1);
        }

        for _ in 0..BLUR_ITERS {
            // H-blur: bloom_a → bloom_b
            {
                let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("bloom_h"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.bloom.view_b,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                rp.set_pipeline(&self.blur_h_pipeline);
                rp.set_bind_group(0, &self.blur_h_bg, &[]);
                rp.draw(0..3, 0..1);
            }
            // V-blur: bloom_b → bloom_a
            {
                let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("bloom_v"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.bloom.view_a,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                rp.set_pipeline(&self.blur_v_pipeline);
                rp.set_bind_group(0, &self.blur_v_bg, &[]);
                rp.draw(0..3, 0..1);
            }
        }

        // Tonemap + FXAA: HDR + bloom_a → swapchain
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tonemap"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: swapchain_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&self.tonemap_pipeline);
            rp.set_bind_group(0, &self.tonemap_bg, &[]);
            rp.draw(0..3, 0..1);
        }
    }
}
