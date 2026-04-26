use std::mem::size_of;

use bytemuck::{Pod, Zeroable};

use crate::render_command::{DrawIndexedIndirectArgs, EntityCullData};

pub const MAX_CULL_ENTITIES: u64 = 4096;

/// Per-frame data for the occlusion-cull compute shader (96 bytes).
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct CullUniforms {
    pub view_proj: [[f32; 4]; 4], // 64
    pub screen_size: [f32; 2],    //  8
    pub entity_count: u32,        //  4
    pub mip_levels: u32,          //  4
    pub cull_enabled: u32,        //  4
    pub _pad: [u32; 3],           // 12
}

/// Per-frame split breakdown of culling rejects (8 bytes / 2 atomics on GPU).
///
/// `[0] = frustum_rejected`, `[1] = hiz_rejected`.  CPU-readable layout so a
/// COPY_SRC → MAP_READ readback decodes directly into `[u32; 2]`.
pub const CULL_STATS_SIZE: u64 = 8;

/// GPU resources for the Hi-Z occlusion-cull compute pass.
pub struct CullPass {
    pipeline: wgpu::ComputePipeline,
    #[allow(dead_code)]
    cull_bg_layout: wgpu::BindGroupLayout,
    pub hiz_bg_layout: wgpu::BindGroupLayout,

    pub cull_uniform_buf: wgpu::Buffer,
    pub entity_cull_buf: wgpu::Buffer,
    /// GPU-written indirect draw args — one `DrawIndexedIndirectArgs` per entity.
    /// instance_count = 0 (culled) or 1 (visible).
    pub entity_indirect_buf: wgpu::Buffer,
    /// 2× u32 atomic counters: `[frustum_rejected, hiz_rejected]`. Cleared to
    /// zero by `clear_stats()` at the start of every frame, incremented by
    /// `atomicAdd` in the cull shader, copied to a readback staging buffer by
    /// the caller for diagnostics. Splits the cull budget into "rejected by
    /// GPU frustum reject" vs. "rejected by Hi-Z occlusion test".
    pub cull_stats_buf: wgpu::Buffer,

    cull_bg: wgpu::BindGroup,
    hiz_bg: Option<wgpu::BindGroup>,
}

impl CullPass {
    pub fn new(device: &wgpu::Device) -> Self {
        // ── group 0: uniforms + entity data + indirect output ─────────────────
        let cull_bg_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cull bg layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(size_of::<CullUniforms>() as u64),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(CULL_STATS_SIZE),
                    },
                    count: None,
                },
            ],
        });

        // ── group 1: Hi-Z texture (all mips, non-filterable) ─────────────────
        let hiz_bg_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cull hiz bg layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });

        // ── buffers ──────────────────────────────────────────────────────────
        let cull_uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cull uniform buf"),
            size: size_of::<CullUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let entity_cull_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("entity cull buf"),
            size: MAX_CULL_ENTITIES * size_of::<EntityCullData>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // STORAGE for GPU writes + INDIRECT for draw_indexed_indirect + COPY_SRC for readback
        let entity_indirect_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("entity indirect buf"),
            size: MAX_CULL_ENTITIES * size_of::<DrawIndexedIndirectArgs>() as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // 2× u32 atomic counters cleared every frame; COPY_SRC enables readback
        // diagnostics, COPY_DST enables the cheap CPU-side `queue.write_buffer`
        // zero-init in `clear_stats()`.
        let cull_stats_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cull stats buf"),
            size: CULL_STATS_SIZE,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let cull_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cull bg"),
            layout: &cull_bg_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: cull_uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: entity_cull_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: entity_indirect_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: cull_stats_buf.as_entire_binding(),
                },
            ],
        });

        // ── pipeline ─────────────────────────────────────────────────────────
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("occlusion cull shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/occlusion_cull.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&cull_bg_layout, &hiz_bg_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("occlusion cull pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("cs_main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Self {
            pipeline,
            cull_bg_layout,
            hiz_bg_layout,
            cull_uniform_buf,
            entity_cull_buf,
            entity_indirect_buf,
            cull_stats_buf,
            cull_bg,
            hiz_bg: None,
        }
    }

    /// Zero the 2× u32 atomic counters before each cull dispatch.
    /// Must be called every frame *before* `encode()`, otherwise `atomicAdd`
    /// in the cull shader keeps accumulating across frames.
    pub fn clear_stats(&self, queue: &wgpu::Queue) {
        let zero: [u32; 2] = [0, 0];
        queue.write_buffer(&self.cull_stats_buf, 0, bytemuck::cast_slice(&zero));
    }

    /// Rebuild the Hi-Z bind group after `HizPass::resize()`.
    pub fn rebuild_hiz_bg(&mut self, device: &wgpu::Device, hiz_full_view: &wgpu::TextureView) {
        self.hiz_bg = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cull hiz bg"),
            layout: &self.hiz_bg_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(hiz_full_view),
            }],
        }));
    }

    /// Encode the occlusion-cull compute dispatch.
    /// Must be called after `HizPass::encode()` in the same command encoder.
    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder, entity_count: u32) {
        let Some(hiz_bg) = &self.hiz_bg else { return };
        let mut cp = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("occlusion cull"),
            timestamp_writes: None,
        });
        cp.set_pipeline(&self.pipeline);
        cp.set_bind_group(0, &self.cull_bg, &[]);
        cp.set_bind_group(1, hiz_bg, &[]);
        cp.dispatch_workgroups(entity_count.div_ceil(64), 1, 1);
    }
}
