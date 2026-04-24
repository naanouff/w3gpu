//! Minimal **Phase B** executor: run a v0 [`w3drs_render_graph::RenderGraphDocument`] on `wgpu`
//! (native only — loads WGSL from disk).
//!
//! Supports the fixture [`fixtures/phases/phase-b/render_graph.json`](../../fixtures/phases/phase-b/render_graph.json):
//! `texture_2d` (couleur + depth optionnelle) + `buffer` resources, puis `compute` puis `raster_mesh`
//! (cible couleur + `depth_target` si présent).

use std::collections::HashMap;
use std::path::Path;

use w3drs_render_graph::{validate_exec_v0, Pass, RenderGraphDocument, Resource};
use wgpu::util::DeviceExt;

#[derive(Debug, thiserror::Error)]
pub enum RenderGraphExecError {
    #[error("unknown texture format {0:?}")]
    UnknownTextureFormat(String),
    #[error("unknown buffer usage flag {0:?}")]
    UnknownBufferUsage(String),
    #[error("unknown texture usage flag {0:?}")]
    UnknownTextureUsage(String),
    #[error("resource id {0:?} not found")]
    MissingResource(String),
    #[error("raster pass {0:?} has no color_targets")]
    EmptyColorTargets(String),
    #[error("v0 checksum readback requires texture {id:?} in format Rgba16Float (got {got:?})")]
    InvalidReadbackFormat { id: String, got: String },
    #[error("raster pass {pass_id:?} lists texture {texture_id:?} twice in color_targets")]
    DuplicateRasterColorTarget {
        pass_id: String,
        texture_id: String,
    },
    #[error("depth_target {texture_id:?} must use a depth/stencil format (got {got:?})")]
    InvalidDepthTargetFormat { texture_id: String, got: String },
    #[error(
        "pass {pass_id:?} requires texture {texture_id:?} to declare usage {missing:?} (has {declared:?})"
    )]
    TextureSemanticUsageMissing {
        pass_id: String,
        texture_id: String,
        missing: String,
        declared: Vec<String>,
    },
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
}

impl From<w3drs_render_graph::RenderGraphValidateError> for RenderGraphExecError {
    fn from(e: w3drs_render_graph::RenderGraphValidateError) -> Self {
        use w3drs_render_graph::RenderGraphValidateError as V;
        match e {
            V::UnknownTextureFormat(s) => Self::UnknownTextureFormat(s),
            V::UnknownBufferUsage(s) => Self::UnknownBufferUsage(s),
            V::UnknownTextureUsage(s) => Self::UnknownTextureUsage(s),
            V::MissingResource(s) => Self::MissingResource(s),
            V::EmptyColorTargets(s) => Self::EmptyColorTargets(s),
            V::InvalidReadbackFormat { id, got } => Self::InvalidReadbackFormat { id, got },
            V::DuplicateRasterColorTarget {
                pass_id,
                texture_id,
            } => Self::DuplicateRasterColorTarget {
                pass_id,
                texture_id,
            },
            V::InvalidDepthTargetFormat { texture_id, got } => {
                Self::InvalidDepthTargetFormat { texture_id, got }
            }
            V::TextureSemanticUsageMissing {
                pass_id,
                texture_id,
                missing,
                declared,
            } => Self::TextureSemanticUsageMissing {
                pass_id,
                texture_id,
                missing,
                declared,
            },
        }
    }
}

fn texture_format(s: &str) -> Result<wgpu::TextureFormat, RenderGraphExecError> {
    match s {
        "Rgba16Float" => Ok(wgpu::TextureFormat::Rgba16Float),
        "Rgba8Unorm" => Ok(wgpu::TextureFormat::Rgba8Unorm),
        "Depth24Plus" => Ok(wgpu::TextureFormat::Depth24Plus),
        "Depth32Float" => Ok(wgpu::TextureFormat::Depth32Float),
        "Depth24PlusStencil8" => Ok(wgpu::TextureFormat::Depth24PlusStencil8),
        "Depth32FloatStencil8" => Ok(wgpu::TextureFormat::Depth32FloatStencil8),
        _ => Err(RenderGraphExecError::UnknownTextureFormat(s.to_string())),
    }
}

fn stencil_ops_for_depth_pass(format: wgpu::TextureFormat) -> Option<wgpu::Operations<u32>> {
    use wgpu::TextureFormat as F;
    match format {
        F::Depth24PlusStencil8 | F::Depth32FloatStencil8 => Some(wgpu::Operations {
            load: wgpu::LoadOp::Clear(0),
            store: wgpu::StoreOp::Discard,
        }),
        _ => None,
    }
}

fn buffer_usage(flags: &[String]) -> Result<wgpu::BufferUsages, RenderGraphExecError> {
    let mut u = wgpu::BufferUsages::empty();
    for s in flags {
        match s.as_str() {
            "storage" => u |= wgpu::BufferUsages::STORAGE,
            "copy_dst" => u |= wgpu::BufferUsages::COPY_DST,
            "copy_src" => u |= wgpu::BufferUsages::COPY_SRC,
            "map_read" => u |= wgpu::BufferUsages::MAP_READ,
            other => return Err(RenderGraphExecError::UnknownBufferUsage(other.to_string())),
        }
    }
    if u.is_empty() {
        u = wgpu::BufferUsages::STORAGE;
    }
    Ok(u)
}

fn texture_usage(flags: &[String]) -> Result<wgpu::TextureUsages, RenderGraphExecError> {
    let mut u = wgpu::TextureUsages::empty();
    for s in flags {
        match s.as_str() {
            "texture_binding" => u |= wgpu::TextureUsages::TEXTURE_BINDING,
            "render_attachment" => u |= wgpu::TextureUsages::RENDER_ATTACHMENT,
            "storage" => u |= wgpu::TextureUsages::STORAGE_BINDING,
            "copy_dst" => u |= wgpu::TextureUsages::COPY_DST,
            "copy_src" => u |= wgpu::TextureUsages::COPY_SRC,
            other => return Err(RenderGraphExecError::UnknownTextureUsage(other.to_string())),
        }
    }
    u |= wgpu::TextureUsages::COPY_SRC;
    if !u.contains(wgpu::TextureUsages::RENDER_ATTACHMENT) {
        u |= wgpu::TextureUsages::RENDER_ATTACHMENT;
    }
    Ok(u)
}

/// GPU objects for every `texture_2d` / `buffer` resource in a v0 [`RenderGraphDocument`].
///
/// Phase **B.1** : named registry (creation + lookup) as the hook for **resize** and future
/// barrier planning between passes — replaces ad-hoc `HashMap`s inside the executor.
#[derive(Debug)]
pub struct RenderGraphGpuRegistry {
    textures: HashMap<String, Texture2dGpu>,
    buffers: HashMap<String, wgpu::Buffer>,
}

/// One declared `texture_2d` after GPU allocation.
#[derive(Debug)]
pub struct Texture2dGpu {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub format: wgpu::TextureFormat,
    pub usage: wgpu::TextureUsages,
    pub width: u32,
    pub height: u32,
}

impl RenderGraphGpuRegistry {
    /// Allocate every resource from `doc` (document order preserved for buffers only in maps).
    pub fn new(device: &wgpu::Device, doc: &RenderGraphDocument) -> Result<Self, RenderGraphExecError> {
        let mut textures = HashMap::new();
        let mut buffers = HashMap::new();
        for r in &doc.resources {
            match r {
                Resource::Texture2d {
                    id,
                    format,
                    width,
                    height,
                    usage,
                } => {
                    let fmt = texture_format(format)?;
                    let usage_wgpu = texture_usage(usage)?;
                    let tex = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some(id.as_str()),
                        size: wgpu::Extent3d {
                            width: *width,
                            height: *height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: fmt,
                        usage: usage_wgpu,
                        view_formats: &[],
                    });
                    let view = tex.create_view(&Default::default());
                    textures.insert(
                        id.clone(),
                        Texture2dGpu {
                            texture: tex,
                            view,
                            format: fmt,
                            usage: usage_wgpu,
                            width: *width,
                            height: *height,
                        },
                    );
                }
                Resource::Buffer { id, size, usage } => {
                    let usage_wgpu = buffer_usage(usage)?;
                    let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(id.as_str()),
                        contents: &vec![0u8; *size as usize],
                        usage: usage_wgpu,
                    });
                    buffers.insert(id.clone(), buf);
                }
            }
        }
        Ok(Self {
            textures,
            buffers,
        })
    }

    pub fn texture_2d(&self, id: &str) -> Result<&Texture2dGpu, RenderGraphExecError> {
        self.textures
            .get(id)
            .ok_or_else(|| RenderGraphExecError::MissingResource(id.to_string()))
    }

    pub fn texture_view(&self, id: &str) -> Result<&wgpu::TextureView, RenderGraphExecError> {
        Ok(&self.texture_2d(id)?.view)
    }

    pub fn texture_format(&self, id: &str) -> Result<wgpu::TextureFormat, RenderGraphExecError> {
        Ok(self.texture_2d(id)?.format)
    }

    pub fn texture_extent(&self, id: &str) -> Result<(u32, u32), RenderGraphExecError> {
        let t = self.texture_2d(id)?;
        Ok((t.width, t.height))
    }

    pub fn texture_wgpu(&self, id: &str) -> Result<&wgpu::Texture, RenderGraphExecError> {
        Ok(&self.texture_2d(id)?.texture)
    }

    pub fn buffer(&self, id: &str) -> Result<&wgpu::Buffer, RenderGraphExecError> {
        self.buffers
            .get(id)
            .ok_or_else(|| RenderGraphExecError::MissingResource(id.to_string()))
    }

    /// Recreate a `texture_2d` at a new size (same format / usage as at creation). No-op if
    /// width and height are unchanged.
    pub fn resize_texture_2d(
        &mut self,
        device: &wgpu::Device,
        id: &str,
        width: u32,
        height: u32,
    ) -> Result<(), RenderGraphExecError> {
        let entry = self.textures.get_mut(id).ok_or_else(|| {
            RenderGraphExecError::MissingResource(format!("resize: unknown texture {id:?}"))
        })?;
        if entry.width == width && entry.height == height {
            return Ok(());
        }
        let fmt = entry.format;
        let usage = entry.usage;
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(id),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: fmt,
            usage,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        *entry = Texture2dGpu {
            texture: tex,
            view,
            format: fmt,
            usage,
            width,
            height,
        };
        Ok(())
    }
}

/// Static checks for v0 execution (formats, usages, pass wiring, readback target).
/// Delegates to [`w3drs_render_graph::validate_exec_v0`] — **no GPU**, runs on WASM too.
pub fn validate_render_graph_exec_v0(
    doc: &RenderGraphDocument,
    readback_id: &str,
) -> Result<(), RenderGraphExecError> {
    validate_exec_v0(doc, readback_id).map_err(Into::into)
}

/// Same as [`run_graph_v0_checksum`] but uses an existing [`RenderGraphGpuRegistry`] (e.g. after
/// [`RenderGraphGpuRegistry::resize_texture_2d`]) so GPU allocation can diverge from the
/// declared `width` / `height` in `doc` until the schema tracks runtime size.
pub fn run_graph_v0_checksum_with_registry(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    doc: &RenderGraphDocument,
    shader_root: &Path,
    readback_id: &str,
    registry: &RenderGraphGpuRegistry,
) -> Result<u64, RenderGraphExecError> {
    validate_render_graph_exec_v0(doc, readback_id)?;

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("render_graph_v0"),
    });

    for pass in &doc.passes {
        match pass {
            Pass::Compute {
                id,
                shader,
                entry_point,
                dispatch,
                ..
            } => {
                let path = shader_root.join(shader);
                let source = std::fs::read_to_string(&path)?;
                let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(id.as_str()),
                    source: wgpu::ShaderSource::Wgsl(source.into()),
                });
                let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("phase_b_compute_empty"),
                    bind_group_layouts: &[],
                    push_constant_ranges: &[],
                });
                let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(id.as_str()),
                    layout: Some(&layout),
                    module: &module,
                    entry_point: Some(entry_point.as_str()),
                    compilation_options: Default::default(),
                    cache: None,
                });
                let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some(id.as_str()),
                    timestamp_writes: None,
                });
                cpass.set_pipeline(&pipeline);
                cpass.dispatch_workgroups(dispatch.x, dispatch.y, dispatch.z);
            }
            Pass::RasterMesh {
                id,
                shader,
                vertex_entry,
                fragment_entry,
                color_targets,
                depth_target,
            } => {
                let first = color_targets
                    .first()
                    .ok_or_else(|| RenderGraphExecError::EmptyColorTargets(id.clone()))?;
                let view = registry.texture_view(first)?;
                let fmt = registry.texture_format(first)?;
                let depth_fmt = depth_target
                    .as_ref()
                    .map(|dt| registry.texture_format(dt))
                    .transpose()?;
                let depth_view = depth_target
                    .as_ref()
                    .map(|dt| registry.texture_view(dt))
                    .transpose()?;
                let path = shader_root.join(shader);
                let source = std::fs::read_to_string(&path)?;
                let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(id.as_str()),
                    source: wgpu::ShaderSource::Wgsl(source.into()),
                });
                let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("phase_b_raster_empty"),
                    bind_group_layouts: &[],
                    push_constant_ranges: &[],
                });
                let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(id.as_str()),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &module,
                        entry_point: Some(vertex_entry.as_str()),
                        buffers: &[],
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &module,
                        entry_point: Some(fragment_entry.as_str()),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: fmt,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        ..Default::default()
                    },
                    depth_stencil: depth_fmt.map(|format| wgpu::DepthStencilState {
                        format,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::LessEqual,
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                    cache: None,
                });
                {
                    let depth_attachment = depth_view.zip(depth_fmt).map(|(dv, df)| {
                        wgpu::RenderPassDepthStencilAttachment {
                            view: dv,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: stencil_ops_for_depth_pass(df),
                        }
                    });
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some(id.as_str()),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: depth_attachment,
                        occlusion_query_set: None,
                        timestamp_writes: None,
                    });
                    rpass.set_pipeline(&pipeline);
                    rpass.draw(0..3, 0..1);
                }
            }
        }
    }

    let (w, h) = registry.texture_extent(readback_id)?;
    let texture = registry.texture_wgpu(readback_id)?;

    let bytes_per_pixel = 8u32; // Rgba16Float
    let unpadded_bytes_per_row = (w * bytes_per_pixel) as usize;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
    let padded_bytes_per_row = unpadded_bytes_per_row.next_multiple_of(align);
    let readback_size = (padded_bytes_per_row as u64) * (h as u64);

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("render_graph_readback"),
        size: readback_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row as u32),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(std::iter::once(encoder.finish()));
    device.poll(wgpu::Maintain::Wait);

    readback.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::Maintain::Wait);
    let slice = readback.slice(..);
    let data = slice.get_mapped_range();
    let sum = fnv1a64(&data);
    drop(data);
    readback.unmap();
    Ok(sum)
}

/// Encode and submit one frame: all passes in document order, then return a **stable checksum**
/// of the `readback_id` texture (`Rgba16Float` only in v0).
pub fn run_graph_v0_checksum(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    doc: &RenderGraphDocument,
    shader_root: &Path,
    readback_id: &str,
) -> Result<u64, RenderGraphExecError> {
    let registry = RenderGraphGpuRegistry::new(device, doc)?;
    run_graph_v0_checksum_with_registry(device, queue, doc, shader_root, readback_id, &registry)
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 14695981039346656037;
    const PRIME: u64 = 1099511628211;
    let mut h = OFFSET;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(PRIME);
    }
    h
}
