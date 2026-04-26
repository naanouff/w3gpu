//! **Phase B** executor: run a v0 [`w3drs_render_graph::RenderGraphDocument`] on `wgpu`.
//! Natif : [`encode_render_graph_passes_v0`] lit le WGSL sur disque. **WASM** / tests : fournir le
//! texte via [`encode_render_graph_passes_v0_with_wgsl`].
//!
//! Supports the fixture [`fixtures/phases/phase-b/render_graph.json`](../../fixtures/phases/phase-b/render_graph.json):
//! `texture_2d` (couleur + depth optionnelle, `mip_level_count`) + `buffer` resources, puis `compute`
//! (`storage_buffers`, `storage_buffers_read`, `storage_writes`, `texture_reads` → group 0 ;
//! `storage_buffers_group1` / `storage_buffers_read_group1` → group 1 ;
//! `indirect_dispatch` optionnel → `dispatch_workgroups_indirect`, buffer usage `indirect`) puis
//! `raster_mesh` / **`fullscreen`** (même encode raster), puis passes **`blit`**
//! (`copy_texture_to_texture`, option `region` pour sous-rectangle et mips).

use std::collections::HashMap;
use std::mem::size_of;
use std::num::NonZeroU64;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use futures_channel::oneshot;
#[cfg(not(target_arch = "wasm32"))]
use futures_executor::block_on;

use w3drs_render_graph::{validate_exec_v0, Pass, RenderGraphDocument, Resource};
use wgpu::util::DeviceExt;

use crate::light_uniforms::LightUniforms;
use crate::vertex_layout::VERTEX_BUFFER_LAYOUT;

// ── B.6 / B.7 : hooks ECS + dessins mesh depth-only (donnée JSON + code hôte) ─────────

/// **B.6** : nœud ECS (label dans le JSON) appelle [`ecs_node`](RenderGraphV0Host::ecs_node) avant/après chaque passe.
/// **B.7** : [`draw_raster_depth_mesh`](RenderGraphV0Host::draw_raster_depth_mesh) encodage des `draw` indexés (ombre, etc.) après bind groups moteur.
pub trait RenderGraphV0Host {
    fn ecs_node(&mut self, _label: &str) {}

    /// Passe `raster_depth_mesh` : pipeline, bind 0/1, buffer de sommets/indices = responsabilité de l’hôte (même modèle que [`ShadowPass`](crate::shadow_pass::ShadowPass) encodé aujourd’hui côté sample).
    fn draw_raster_depth_mesh(&mut self, _pass_id: &str, _rpass: &mut wgpu::RenderPass<'_>) {}
}

/// Comportement historique (B.1–B.5) : ignorer les nœuds ECS / passes depth mesh host.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopRenderGraphV0Host;
impl RenderGraphV0Host for NoopRenderGraphV0Host {}

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
    DuplicateRasterColorTarget { pass_id: String, texture_id: String },
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
    #[error(
        "pass {pass_id:?} requires buffer {buffer_id:?} to declare usage {missing:?} (has {declared:?})"
    )]
    BufferSemanticUsageMissing {
        pass_id: String,
        buffer_id: String,
        missing: String,
        declared: Vec<String>,
    },
    #[error("compute pass {pass_id:?} lists buffer {buffer_id:?} twice in storage_buffers")]
    DuplicateComputeStorageBuffer { pass_id: String, buffer_id: String },
    #[error("compute pass {pass_id:?} lists buffer {buffer_id:?} twice in storage_buffers_read")]
    DuplicateComputeStorageBufferRead { pass_id: String, buffer_id: String },
    #[error(
        "compute pass {pass_id:?}: buffer {buffer_id:?} appears in both storage_buffers and storage_buffers_read"
    )]
    ComputeStorageBufferRwRoConflict { pass_id: String, buffer_id: String },
    #[error(
        "compute pass {pass_id:?}: bind group 1 lists buffers but bind group 0 has no resources"
    )]
    ComputeGroup1RequiresGroup0 { pass_id: String },
    #[error("compute pass {pass_id:?} lists buffer {buffer_id:?} twice in storage_buffers_group1")]
    DuplicateComputeStorageBufferGroup1 { pass_id: String, buffer_id: String },
    #[error(
        "compute pass {pass_id:?} lists buffer {buffer_id:?} twice in storage_buffers_read_group1"
    )]
    DuplicateComputeStorageBufferReadGroup1 { pass_id: String, buffer_id: String },
    #[error(
        "compute pass {pass_id:?}: buffer {buffer_id:?} appears in both storage_buffers_group1 and storage_buffers_read_group1"
    )]
    ComputeStorageBufferGroup1RwRoConflict { pass_id: String, buffer_id: String },
    #[error(
        "compute pass {pass_id:?}: buffer {buffer_id:?} appears in bind group 0 and bind group 1 buffer lists"
    )]
    ComputeBufferSharedAcrossBindGroups { pass_id: String, buffer_id: String },
    #[error("compute pass {pass_id:?} lists texture {texture_id:?} twice in storage_writes")]
    DuplicateComputeStorageTexture { pass_id: String, texture_id: String },
    #[error("compute pass {pass_id:?} lists texture {texture_id:?} twice in texture_reads")]
    DuplicateComputeReadTexture { pass_id: String, texture_id: String },
    #[error(
        "compute pass {pass_id:?}: texture {texture_id:?} appears in both texture_reads and storage_writes"
    )]
    ComputeTextureReadWriteConflict { pass_id: String, texture_id: String },
    #[error(
        "compute pass {pass_id:?}: texture_reads format {got:?} for {texture_id:?} is not supported in v0 (use Rgba16Float or Rgba8Unorm)"
    )]
    InvalidComputeTextureReadFormat {
        pass_id: String,
        texture_id: String,
        got: String,
    },
    #[error("compute texture_reads: sampled binding not implemented for format {0:?}")]
    UnsupportedComputeTextureReadFormat(String),
    #[error("blit pass {pass_id:?}: source and destination must differ (got {texture_id:?})")]
    BlitSameTexture { pass_id: String, texture_id: String },
    #[error(
        "blit pass {pass_id:?}: texture {src_id:?} format {src_fmt:?} != texture {dst_id:?} format {dst_fmt:?}"
    )]
    BlitFormatMismatch {
        pass_id: String,
        src_id: String,
        dst_id: String,
        src_fmt: String,
        dst_fmt: String,
    },
    #[error(
        "blit pass {pass_id:?}: texture {src_id:?} size {sw}x{sh} != texture {dst_id:?} size {dw}x{dh}"
    )]
    BlitExtentMismatch {
        pass_id: String,
        src_id: String,
        dst_id: String,
        sw: u32,
        sh: u32,
        dw: u32,
        dh: u32,
    },
    #[error("texture {id:?}: mip_level_count must be in 1..=32 (got {got})")]
    InvalidTextureMipLevelCount { id: String, got: u32 },
    #[error(
        "blit pass {pass_id:?}: copy subregion exceeds texture bounds or has zero size (source {src_id:?}, dest {dst_id:?})"
    )]
    BlitRegionInvalid {
        pass_id: String,
        src_id: String,
        dst_id: String,
    },
    #[error(
        "compute pass {pass_id:?}: indirect_dispatch buffer {buffer_id:?} must declare usage {missing:?} (has {declared:?})"
    )]
    IndirectBufferUsageMissing {
        pass_id: String,
        buffer_id: String,
        missing: String,
        declared: Vec<String>,
    },
    #[error(
        "compute pass {pass_id:?}: indirect_dispatch offset {offset} + 12 exceeds buffer {buffer_id:?} size {size}"
    )]
    IndirectDispatchOutOfBuffer {
        pass_id: String,
        buffer_id: String,
        offset: u64,
        size: u64,
    },
    #[error(
        "compute pass {pass_id:?}: indirect_dispatch offset {offset} must be a multiple of 4 for buffer {buffer_id:?}"
    )]
    IndirectDispatchOffsetMisaligned {
        pass_id: String,
        buffer_id: String,
        offset: u64,
    },
    #[error("pass {pass_id:?} must omit `{field}` or set a non-empty string (B.6 ECS)")]
    EcsLabelEmpty {
        pass_id: String,
        field: &'static str,
    },
    #[error("raster_depth_mesh pass {pass_id:?}: {detail}")]
    RasterDepthMeshInvalid { pass_id: String, detail: String },
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("WGSL not provided for {rel:?} (in-memory loader)")]
    WgslNotFound { rel: String },
    #[error("v0 readback: buffer map oneshot dropped (internal)")]
    ReadbackMapOneshotDropped,
    #[error("v0 readback: buffer map async failed: {0}")]
    ReadbackBufferMapFailed(#[source] wgpu::BufferAsyncError),
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
            V::BufferSemanticUsageMissing {
                pass_id,
                buffer_id,
                missing,
                declared,
            } => Self::BufferSemanticUsageMissing {
                pass_id,
                buffer_id,
                missing,
                declared,
            },
            V::DuplicateComputeStorageBuffer { pass_id, buffer_id } => {
                Self::DuplicateComputeStorageBuffer { pass_id, buffer_id }
            }
            V::DuplicateComputeStorageBufferRead { pass_id, buffer_id } => {
                Self::DuplicateComputeStorageBufferRead { pass_id, buffer_id }
            }
            V::ComputeStorageBufferRwRoConflict { pass_id, buffer_id } => {
                Self::ComputeStorageBufferRwRoConflict { pass_id, buffer_id }
            }
            V::ComputeGroup1RequiresGroup0 { pass_id } => {
                Self::ComputeGroup1RequiresGroup0 { pass_id }
            }
            V::DuplicateComputeStorageBufferGroup1 { pass_id, buffer_id } => {
                Self::DuplicateComputeStorageBufferGroup1 { pass_id, buffer_id }
            }
            V::DuplicateComputeStorageBufferReadGroup1 { pass_id, buffer_id } => {
                Self::DuplicateComputeStorageBufferReadGroup1 { pass_id, buffer_id }
            }
            V::ComputeStorageBufferGroup1RwRoConflict { pass_id, buffer_id } => {
                Self::ComputeStorageBufferGroup1RwRoConflict { pass_id, buffer_id }
            }
            V::ComputeBufferSharedAcrossBindGroups { pass_id, buffer_id } => {
                Self::ComputeBufferSharedAcrossBindGroups { pass_id, buffer_id }
            }
            V::DuplicateComputeStorageTexture {
                pass_id,
                texture_id,
            } => Self::DuplicateComputeStorageTexture {
                pass_id,
                texture_id,
            },
            V::DuplicateComputeReadTexture {
                pass_id,
                texture_id,
            } => Self::DuplicateComputeReadTexture {
                pass_id,
                texture_id,
            },
            V::ComputeTextureReadWriteConflict {
                pass_id,
                texture_id,
            } => Self::ComputeTextureReadWriteConflict {
                pass_id,
                texture_id,
            },
            V::InvalidComputeTextureReadFormat {
                pass_id,
                texture_id,
                got,
            } => Self::InvalidComputeTextureReadFormat {
                pass_id,
                texture_id,
                got,
            },
            V::BlitSameTexture {
                pass_id,
                texture_id,
            } => Self::BlitSameTexture {
                pass_id,
                texture_id,
            },
            V::BlitFormatMismatch {
                pass_id,
                src_id,
                dst_id,
                src_fmt,
                dst_fmt,
            } => Self::BlitFormatMismatch {
                pass_id,
                src_id,
                dst_id,
                src_fmt,
                dst_fmt,
            },
            V::BlitExtentMismatch {
                pass_id,
                src_id,
                dst_id,
                sw,
                sh,
                dw,
                dh,
            } => Self::BlitExtentMismatch {
                pass_id,
                src_id,
                dst_id,
                sw,
                sh,
                dw,
                dh,
            },
            V::InvalidTextureMipLevelCount { id, got } => {
                Self::InvalidTextureMipLevelCount { id, got }
            }
            V::BlitRegionInvalid {
                pass_id,
                src_id,
                dst_id,
            } => Self::BlitRegionInvalid {
                pass_id,
                src_id,
                dst_id,
            },
            V::IndirectBufferUsageMissing {
                pass_id,
                buffer_id,
                missing,
                declared,
            } => Self::IndirectBufferUsageMissing {
                pass_id,
                buffer_id,
                missing,
                declared,
            },
            V::IndirectDispatchOutOfBuffer {
                pass_id,
                buffer_id,
                offset,
                size,
            } => Self::IndirectDispatchOutOfBuffer {
                pass_id,
                buffer_id,
                offset,
                size,
            },
            V::IndirectDispatchOffsetMisaligned {
                pass_id,
                buffer_id,
                offset,
            } => Self::IndirectDispatchOffsetMisaligned {
                pass_id,
                buffer_id,
                offset,
            },
            V::EcsLabelEmpty { pass_id, field } => Self::EcsLabelEmpty { pass_id, field },
            V::RasterDepthMeshInvalid { pass_id, detail } => {
                Self::RasterDepthMeshInvalid { pass_id, detail }
            }
        }
    }
}

fn mip_dimensions(width: u32, height: u32, level: u32) -> (u32, u32) {
    let mut w = width;
    let mut h = height;
    for _ in 0..level {
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }
    (w, h)
}

fn buffer_decl_size_bytes(
    doc: &RenderGraphDocument,
    id: &str,
) -> Result<u64, RenderGraphExecError> {
    for r in &doc.resources {
        if let Resource::Buffer { id: bid, size, .. } = r {
            if bid == id {
                return Ok(*size);
            }
        }
    }
    Err(RenderGraphExecError::MissingResource(id.to_string()))
}

fn texture_sample_type_for_compute_read(
    format: wgpu::TextureFormat,
) -> Result<wgpu::TextureSampleType, RenderGraphExecError> {
    use wgpu::TextureFormat as F;
    match format {
        F::Rgba16Float | F::Rgba8Unorm => Ok(wgpu::TextureSampleType::Float { filterable: false }),
        other => Err(RenderGraphExecError::UnsupportedComputeTextureReadFormat(
            format!("{other:?}"),
        )),
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

fn pass_requests_transparency(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    id.contains("transparent") || id.contains("transparency") || id.contains("alpha")
}

#[allow(clippy::too_many_arguments)]
fn encode_raster_like_pass(
    encoder: &mut wgpu::CommandEncoder,
    device: &wgpu::Device,
    registry: &RenderGraphGpuRegistry,
    load_wgsl: &mut impl FnMut(&str) -> Result<String, RenderGraphExecError>,
    id: &str,
    shader: &str,
    vertex_entry: &str,
    fragment_entry: &str,
    color_targets: &[String],
    depth_target: &Option<String>,
) -> Result<(), RenderGraphExecError> {
    let first = color_targets
        .first()
        .ok_or_else(|| RenderGraphExecError::EmptyColorTargets(id.to_string()))?;
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
    let source = load_wgsl(shader)?;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(id),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("phase_b_raster_empty"),
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });
    let transparency = pass_requests_transparency(id);
    let blend = if transparency {
        wgpu::BlendState::ALPHA_BLENDING
    } else {
        wgpu::BlendState::REPLACE
    };
    let color_load = if transparency {
        wgpu::LoadOp::Load
    } else {
        wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
    };

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(id),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some(vertex_entry),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some(fragment_entry),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: fmt,
                blend: Some(blend),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: depth_fmt.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: !transparency,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });
    let depth_attachment =
        depth_view
            .zip(depth_fmt)
            .map(|(dv, df)| wgpu::RenderPassDepthStencilAttachment {
                view: dv,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: stencil_ops_for_depth_pass(df),
            });
    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(id),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: color_load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: depth_attachment,
        occlusion_query_set: None,
        timestamp_writes: None,
    });
    rpass.set_pipeline(&pipeline);
    rpass.draw(0..3, 0..1);
    Ok(())
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

/// **B.7** — Passe `raster_depth_mesh` (WGSL `shadow_depth`-compatible : group0 uniform, group1 instances).
#[allow(clippy::too_many_arguments)]
fn encode_raster_depth_mesh_pass(
    encoder: &mut wgpu::CommandEncoder,
    device: &wgpu::Device,
    doc: &RenderGraphDocument,
    registry: &RenderGraphGpuRegistry,
    load_wgsl: &mut impl FnMut(&str) -> Result<String, RenderGraphExecError>,
    id: &str,
    shader: &str,
    vertex_entry: &str,
    depth_target: &str,
    light_u: &str,
    inst: &str,
    host: &mut impl RenderGraphV0Host,
) -> Result<(), RenderGraphExecError> {
    let inst_size = buffer_decl_size_bytes(doc, inst)?;
    let u_min = NonZeroU64::new(size_of::<LightUniforms>() as u64).ok_or_else(|| {
        RenderGraphExecError::RasterDepthMeshInvalid {
            pass_id: id.to_string(),
            detail: "internal: LightUniforms size is zero".into(),
        }
    })?;
    let s_min = NonZeroU64::new(inst_size.max(4)).ok_or_else(|| {
        RenderGraphExecError::RasterDepthMeshInvalid {
            pass_id: id.to_string(),
            detail: "instance buffer declared size is zero".into(),
        }
    })?;

    let light_buf = registry.buffer(light_u)?;
    let inst_buf = registry.buffer(inst)?;
    let depth_view = registry.texture_view(depth_target)?;
    let depth_fmt = registry.texture_format(depth_target)?;
    let source = load_wgsl(shader)?;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(id),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let shadow_light_bg_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rg_depth_mesh g0"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(u_min),
                },
                count: None,
            }],
        });
    let inst_bg_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("rg_depth_mesh g1"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: Some(s_min),
            },
            count: None,
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("rg_depth_mesh pl"),
        bind_group_layouts: &[&shadow_light_bg_layout, &inst_bg_layout],
        push_constant_ranges: &[],
    });
    let depth_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("rg_depth_mesh"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some(vertex_entry),
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
            format: depth_fmt,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
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
    let light_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("rg_depth_mesh bg0"),
        layout: &shadow_light_bg_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: light_buf.as_entire_binding(),
        }],
    });
    let inst_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("rg_depth_mesh bg1"),
        layout: &inst_bg_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: inst_buf,
                offset: 0,
                size: None,
            }),
        }],
    });
    let depth_attach = wgpu::RenderPassDepthStencilAttachment {
        view: depth_view,
        depth_ops: Some(wgpu::Operations {
            load: wgpu::LoadOp::Clear(1.0),
            store: wgpu::StoreOp::Store,
        }),
        stencil_ops: stencil_ops_for_depth_pass(depth_fmt),
    };
    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(id),
        color_attachments: &[],
        depth_stencil_attachment: Some(depth_attach),
        occlusion_query_set: None,
        timestamp_writes: None,
    });
    rpass.set_pipeline(&depth_pipeline);
    rpass.set_bind_group(0, &light_bg, &[]);
    rpass.set_bind_group(1, &inst_bg, &[]);
    host.draw_raster_depth_mesh(id, &mut rpass);
    Ok(())
}

fn buffer_usage(flags: &[String]) -> Result<wgpu::BufferUsages, RenderGraphExecError> {
    let mut u = wgpu::BufferUsages::empty();
    for s in flags {
        match s.as_str() {
            "storage" => u |= wgpu::BufferUsages::STORAGE,
            "copy_dst" => u |= wgpu::BufferUsages::COPY_DST,
            "copy_src" => u |= wgpu::BufferUsages::COPY_SRC,
            "map_read" => u |= wgpu::BufferUsages::MAP_READ,
            "indirect" => u |= wgpu::BufferUsages::INDIRECT,
            "uniform" => u |= wgpu::BufferUsages::UNIFORM,
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
    pub mip_level_count: u32,
}

impl RenderGraphGpuRegistry {
    /// Allocate every resource from `doc` (document order preserved for buffers only in maps).
    pub fn new(
        device: &wgpu::Device,
        doc: &RenderGraphDocument,
    ) -> Result<Self, RenderGraphExecError> {
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
                    mip_level_count,
                } => {
                    let fmt = texture_format(format)?;
                    let usage_wgpu = texture_usage(usage)?;
                    let mips = (*mip_level_count).clamp(1, 32);
                    let tex = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some(id.as_str()),
                        size: wgpu::Extent3d {
                            width: *width,
                            height: *height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: mips,
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
                            mip_level_count: mips,
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
        Ok(Self { textures, buffers })
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

    /// Remplace / insère un buffer nommé (B.7 : câblage moteur → noms du JSON).
    pub fn insert_buffer(&mut self, id: String, buffer: wgpu::Buffer) {
        self.buffers.insert(id, buffer);
    }

    /// Remplace / insère une texture 2D nommée (B.7 : câblage ombre moteur → ressource graphe).
    pub fn insert_texture_2d(&mut self, id: String, gpu: Texture2dGpu) {
        self.textures.insert(id, gpu);
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
        let mips = entry.mip_level_count;
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(id),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: mips,
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
            mip_level_count: mips,
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

/// Encode every pass in `doc` in order (compute, raster, fullscreen, blit, `raster_depth_mesh`) into `encoder`.
///
/// Même sémantique que [`encode_render_graph_passes_v0_with_wgsl_host`] sans hooks **B.6** / **B.7**.
pub fn encode_render_graph_passes_v0_with_wgsl(
    encoder: &mut wgpu::CommandEncoder,
    device: &wgpu::Device,
    registry: &RenderGraphGpuRegistry,
    doc: &RenderGraphDocument,
    load_wgsl: &mut impl FnMut(&str) -> Result<String, RenderGraphExecError>,
) -> Result<(), RenderGraphExecError> {
    let mut noop = NoopRenderGraphV0Host;
    encode_render_graph_passes_v0_with_wgsl_host(
        encoder, device, registry, doc, load_wgsl, &mut noop,
    )
}

/// Même paramètres + **hôte** (ECs `ecs_before` / `ecs_after`, `draw_raster_depth_mesh`).
pub fn encode_render_graph_passes_v0_with_wgsl_host(
    encoder: &mut wgpu::CommandEncoder,
    device: &wgpu::Device,
    registry: &RenderGraphGpuRegistry,
    doc: &RenderGraphDocument,
    load_wgsl: &mut impl FnMut(&str) -> Result<String, RenderGraphExecError>,
    host: &mut impl RenderGraphV0Host,
) -> Result<(), RenderGraphExecError> {
    for pass in &doc.passes {
        if let Some(l) = pass.ecs_before_label() {
            host.ecs_node(l);
        }
        match pass {
            Pass::Compute {
                id,
                shader,
                entry_point,
                dispatch,
                texture_reads,
                storage_buffers,
                storage_buffers_read,
                storage_writes,
                storage_buffers_group1,
                storage_buffers_read_group1,
                indirect_dispatch,
                ..
            } => {
                let source = load_wgsl(shader.as_str())?;
                let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(id.as_str()),
                    source: wgpu::ShaderSource::Wgsl(source.into()),
                });

                let buf_rw = storage_buffers.len();
                let buf_ro = storage_buffers_read.len();
                let st_n = storage_writes.len();
                let read_n = texture_reads.len();
                let bind_n = buf_rw + buf_ro + st_n + read_n;
                let buf_g1_rw = storage_buffers_group1.len();
                let buf_g1_ro = storage_buffers_read_group1.len();
                let g1_n = buf_g1_rw + buf_g1_ro;

                let (pipeline_layout, bind_group0, bind_group1) = if bind_n == 0 && g1_n == 0 {
                    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("phase_b_compute_empty"),
                        bind_group_layouts: &[],
                        push_constant_ranges: &[],
                    });
                    (pl, None, None)
                } else if bind_n > 0 && g1_n == 0 {
                    let mut layout_entries = Vec::with_capacity(bind_n);
                    for (binding, bid) in storage_buffers.iter().enumerate() {
                        let size = buffer_decl_size_bytes(doc, bid)?;
                        let min = NonZeroU64::new(size.max(4)).ok_or_else(|| {
                            RenderGraphExecError::MissingResource(format!(
                                "buffer {bid:?}: declared size must be >= 4 for storage binding"
                            ))
                        })?;
                        layout_entries.push(wgpu::BindGroupLayoutEntry {
                            binding: binding as u32,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: Some(min),
                            },
                            count: None,
                        });
                    }
                    for (i, bid) in storage_buffers_read.iter().enumerate() {
                        let binding = (buf_rw + i) as u32;
                        let size = buffer_decl_size_bytes(doc, bid)?;
                        let min = NonZeroU64::new(size.max(4)).ok_or_else(|| {
                            RenderGraphExecError::MissingResource(format!(
                                "buffer {bid:?}: declared size must be >= 4 for storage binding"
                            ))
                        })?;
                        layout_entries.push(wgpu::BindGroupLayoutEntry {
                            binding,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: Some(min),
                            },
                            count: None,
                        });
                    }
                    for (i, tid) in storage_writes.iter().enumerate() {
                        let binding = (buf_rw + buf_ro + i) as u32;
                        let fmt = registry.texture_format(tid)?;
                        layout_entries.push(wgpu::BindGroupLayoutEntry {
                            binding,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::StorageTexture {
                                access: wgpu::StorageTextureAccess::WriteOnly,
                                format: fmt,
                                view_dimension: wgpu::TextureViewDimension::D2,
                            },
                            count: None,
                        });
                    }
                    for (i, tid) in texture_reads.iter().enumerate() {
                        let binding = (buf_rw + buf_ro + st_n + i) as u32;
                        let fmt = registry.texture_format(tid)?;
                        let sample_type = texture_sample_type_for_compute_read(fmt)?;
                        layout_entries.push(wgpu::BindGroupLayoutEntry {
                            binding,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type,
                            },
                            count: None,
                        });
                    }
                    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("phase_b_compute_storage_bgl"),
                        entries: &layout_entries,
                    });
                    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("phase_b_compute_storage"),
                        bind_group_layouts: &[&bgl],
                        push_constant_ranges: &[],
                    });
                    let mut entries = Vec::with_capacity(bind_n);
                    for (binding, bid) in storage_buffers.iter().enumerate() {
                        entries.push(wgpu::BindGroupEntry {
                            binding: binding as u32,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: registry.buffer(bid)?,
                                offset: 0,
                                size: None,
                            }),
                        });
                    }
                    for (i, bid) in storage_buffers_read.iter().enumerate() {
                        let binding = (buf_rw + i) as u32;
                        entries.push(wgpu::BindGroupEntry {
                            binding,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: registry.buffer(bid)?,
                                offset: 0,
                                size: None,
                            }),
                        });
                    }
                    for (i, tid) in storage_writes.iter().enumerate() {
                        let binding = (buf_rw + buf_ro + i) as u32;
                        entries.push(wgpu::BindGroupEntry {
                            binding,
                            resource: wgpu::BindingResource::TextureView(
                                registry.texture_view(tid)?,
                            ),
                        });
                    }
                    for (i, tid) in texture_reads.iter().enumerate() {
                        let binding = (buf_rw + buf_ro + st_n + i) as u32;
                        entries.push(wgpu::BindGroupEntry {
                            binding,
                            resource: wgpu::BindingResource::TextureView(
                                registry.texture_view(tid)?,
                            ),
                        });
                    }
                    let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("phase_b_compute_bg"),
                        layout: &bgl,
                        entries: &entries,
                    });
                    (pl, Some(bg), None)
                } else if bind_n > 0 && g1_n > 0 {
                    let mut layout_entries = Vec::with_capacity(bind_n);
                    for (binding, bid) in storage_buffers.iter().enumerate() {
                        let size = buffer_decl_size_bytes(doc, bid)?;
                        let min = NonZeroU64::new(size.max(4)).ok_or_else(|| {
                            RenderGraphExecError::MissingResource(format!(
                                "buffer {bid:?}: declared size must be >= 4 for storage binding"
                            ))
                        })?;
                        layout_entries.push(wgpu::BindGroupLayoutEntry {
                            binding: binding as u32,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: Some(min),
                            },
                            count: None,
                        });
                    }
                    for (i, bid) in storage_buffers_read.iter().enumerate() {
                        let binding = (buf_rw + i) as u32;
                        let size = buffer_decl_size_bytes(doc, bid)?;
                        let min = NonZeroU64::new(size.max(4)).ok_or_else(|| {
                            RenderGraphExecError::MissingResource(format!(
                                "buffer {bid:?}: declared size must be >= 4 for storage binding"
                            ))
                        })?;
                        layout_entries.push(wgpu::BindGroupLayoutEntry {
                            binding,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: Some(min),
                            },
                            count: None,
                        });
                    }
                    for (i, tid) in storage_writes.iter().enumerate() {
                        let binding = (buf_rw + buf_ro + i) as u32;
                        let fmt = registry.texture_format(tid)?;
                        layout_entries.push(wgpu::BindGroupLayoutEntry {
                            binding,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::StorageTexture {
                                access: wgpu::StorageTextureAccess::WriteOnly,
                                format: fmt,
                                view_dimension: wgpu::TextureViewDimension::D2,
                            },
                            count: None,
                        });
                    }
                    for (i, tid) in texture_reads.iter().enumerate() {
                        let binding = (buf_rw + buf_ro + st_n + i) as u32;
                        let fmt = registry.texture_format(tid)?;
                        let sample_type = texture_sample_type_for_compute_read(fmt)?;
                        layout_entries.push(wgpu::BindGroupLayoutEntry {
                            binding,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type,
                            },
                            count: None,
                        });
                    }
                    let bgl0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("phase_b_compute_g0_bgl"),
                        entries: &layout_entries,
                    });

                    let mut g1_layout = Vec::with_capacity(g1_n);
                    for (binding, bid) in storage_buffers_group1.iter().enumerate() {
                        let size = buffer_decl_size_bytes(doc, bid)?;
                        let min = NonZeroU64::new(size.max(4)).ok_or_else(|| {
                            RenderGraphExecError::MissingResource(format!(
                                "buffer {bid:?}: declared size must be >= 4 for storage binding"
                            ))
                        })?;
                        g1_layout.push(wgpu::BindGroupLayoutEntry {
                            binding: binding as u32,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: Some(min),
                            },
                            count: None,
                        });
                    }
                    for (i, bid) in storage_buffers_read_group1.iter().enumerate() {
                        let binding = (buf_g1_rw + i) as u32;
                        let size = buffer_decl_size_bytes(doc, bid)?;
                        let min = NonZeroU64::new(size.max(4)).ok_or_else(|| {
                            RenderGraphExecError::MissingResource(format!(
                                "buffer {bid:?}: declared size must be >= 4 for storage binding"
                            ))
                        })?;
                        g1_layout.push(wgpu::BindGroupLayoutEntry {
                            binding,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: Some(min),
                            },
                            count: None,
                        });
                    }
                    let bgl1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("phase_b_compute_g1_bgl"),
                        entries: &g1_layout,
                    });

                    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("phase_b_compute_two_groups"),
                        bind_group_layouts: &[&bgl0, &bgl1],
                        push_constant_ranges: &[],
                    });

                    let mut entries0 = Vec::with_capacity(bind_n);
                    for (binding, bid) in storage_buffers.iter().enumerate() {
                        entries0.push(wgpu::BindGroupEntry {
                            binding: binding as u32,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: registry.buffer(bid)?,
                                offset: 0,
                                size: None,
                            }),
                        });
                    }
                    for (i, bid) in storage_buffers_read.iter().enumerate() {
                        let binding = (buf_rw + i) as u32;
                        entries0.push(wgpu::BindGroupEntry {
                            binding,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: registry.buffer(bid)?,
                                offset: 0,
                                size: None,
                            }),
                        });
                    }
                    for (i, tid) in storage_writes.iter().enumerate() {
                        let binding = (buf_rw + buf_ro + i) as u32;
                        entries0.push(wgpu::BindGroupEntry {
                            binding,
                            resource: wgpu::BindingResource::TextureView(
                                registry.texture_view(tid)?,
                            ),
                        });
                    }
                    for (i, tid) in texture_reads.iter().enumerate() {
                        let binding = (buf_rw + buf_ro + st_n + i) as u32;
                        entries0.push(wgpu::BindGroupEntry {
                            binding,
                            resource: wgpu::BindingResource::TextureView(
                                registry.texture_view(tid)?,
                            ),
                        });
                    }
                    let bg0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("phase_b_compute_bg0"),
                        layout: &bgl0,
                        entries: &entries0,
                    });

                    let mut entries1 = Vec::with_capacity(g1_n);
                    for (binding, bid) in storage_buffers_group1.iter().enumerate() {
                        entries1.push(wgpu::BindGroupEntry {
                            binding: binding as u32,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: registry.buffer(bid)?,
                                offset: 0,
                                size: None,
                            }),
                        });
                    }
                    for (i, bid) in storage_buffers_read_group1.iter().enumerate() {
                        let binding = (buf_g1_rw + i) as u32;
                        entries1.push(wgpu::BindGroupEntry {
                            binding,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: registry.buffer(bid)?,
                                offset: 0,
                                size: None,
                            }),
                        });
                    }
                    let bg1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("phase_b_compute_bg1"),
                        layout: &bgl1,
                        entries: &entries1,
                    });
                    (pl, Some(bg0), Some(bg1))
                } else {
                    return Err(RenderGraphExecError::MissingResource(format!(
                        "compute pass {id:?}: bind group 1 buffers require group-0 bindings (validate_exec_v0 should catch this)"
                    )));
                };

                let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(id.as_str()),
                    layout: Some(&pipeline_layout),
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
                if let Some(ref bg) = bind_group0 {
                    cpass.set_bind_group(0, bg, &[]);
                }
                if let Some(ref bg) = bind_group1 {
                    cpass.set_bind_group(1, bg, &[]);
                }
                if let Some(ind) = indirect_dispatch {
                    let buf = registry.buffer(&ind.buffer)?;
                    cpass.dispatch_workgroups_indirect(buf, ind.offset);
                } else {
                    cpass.dispatch_workgroups(dispatch.x, dispatch.y, dispatch.z);
                }
            }
            Pass::RasterMesh {
                id,
                shader,
                vertex_entry,
                fragment_entry,
                color_targets,
                depth_target,
                ..
            } => {
                encode_raster_like_pass(
                    encoder,
                    device,
                    registry,
                    load_wgsl,
                    id,
                    shader,
                    vertex_entry,
                    fragment_entry,
                    color_targets,
                    depth_target,
                )?;
            }
            Pass::Fullscreen {
                id,
                shader,
                vertex_entry,
                fragment_entry,
                color_targets,
                depth_target,
                ..
            } => {
                encode_raster_like_pass(
                    encoder,
                    device,
                    registry,
                    load_wgsl,
                    id,
                    shader,
                    vertex_entry,
                    fragment_entry,
                    color_targets,
                    depth_target,
                )?;
            }
            Pass::Blit {
                id: _,
                source,
                destination,
                region,
                ..
            } => {
                let src_tex = registry.texture_wgpu(source)?;
                let dst_tex = registry.texture_wgpu(destination)?;
                let src_entry = registry.texture_2d(source)?;
                let dst_entry = registry.texture_2d(destination)?;
                let (src_mip, dst_mip, src_origin, dst_origin, extent) = match region.as_ref() {
                    None => {
                        let (w, h) = registry.texture_extent(source)?;
                        (
                            0u32,
                            0u32,
                            wgpu::Origin3d::ZERO,
                            wgpu::Origin3d::ZERO,
                            wgpu::Extent3d {
                                width: w,
                                height: h,
                                depth_or_array_layers: 1,
                            },
                        )
                    }
                    Some(reg) => {
                        let (sw, sh) =
                            mip_dimensions(src_entry.width, src_entry.height, reg.src_mip_level);
                        let (dw, dh) =
                            mip_dimensions(dst_entry.width, dst_entry.height, reg.dst_mip_level);
                        let max_w = (sw - reg.src_origin_x).min(dw - reg.dst_origin_x);
                        let max_h = (sh - reg.src_origin_y).min(dh - reg.dst_origin_y);
                        let copy_w = reg.width.unwrap_or(max_w).min(max_w);
                        let copy_h = reg.height.unwrap_or(max_h).min(max_h);
                        (
                            reg.src_mip_level,
                            reg.dst_mip_level,
                            wgpu::Origin3d {
                                x: reg.src_origin_x,
                                y: reg.src_origin_y,
                                z: 0,
                            },
                            wgpu::Origin3d {
                                x: reg.dst_origin_x,
                                y: reg.dst_origin_y,
                                z: 0,
                            },
                            wgpu::Extent3d {
                                width: copy_w,
                                height: copy_h,
                                depth_or_array_layers: 1,
                            },
                        )
                    }
                };
                encoder.copy_texture_to_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: src_tex,
                        mip_level: src_mip,
                        origin: src_origin,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyTextureInfo {
                        texture: dst_tex,
                        mip_level: dst_mip,
                        origin: dst_origin,
                        aspect: wgpu::TextureAspect::All,
                    },
                    extent,
                );
            }
            Pass::RasterDepthMesh {
                id,
                shader,
                vertex_entry,
                depth_target,
                light_uniforms_buffer,
                instance_buffer,
                ..
            } => {
                encode_raster_depth_mesh_pass(
                    encoder,
                    device,
                    doc,
                    registry,
                    load_wgsl,
                    id.as_str(),
                    shader,
                    vertex_entry,
                    depth_target,
                    light_uniforms_buffer,
                    instance_buffer,
                    host,
                )?;
            }
        }
        if let Some(l) = pass.ecs_after_label() {
            host.ecs_node(l);
        }
    }

    Ok(())
}

/// Native: load WGSL with `std::fs` from `shader_root` (each JSON `shader` path is relative to it).
#[cfg(not(target_arch = "wasm32"))]
pub fn encode_render_graph_passes_v0(
    encoder: &mut wgpu::CommandEncoder,
    device: &wgpu::Device,
    registry: &RenderGraphGpuRegistry,
    doc: &RenderGraphDocument,
    shader_root: &Path,
) -> Result<(), RenderGraphExecError> {
    let mut load = |rel: &str| {
        std::fs::read_to_string(shader_root.join(rel)).map_err(RenderGraphExecError::from)
    };
    encode_render_graph_passes_v0_with_wgsl(encoder, device, registry, doc, &mut load)
}

/// Full graph run (encode + readback checksum) with a **custom** WGSL resolver (WASM, tests, or
/// in-memory maps), hooks **B.6** / **B.7** (`RenderGraphV0Host`). Même sémantique que
/// [`run_graph_v0_checksum_with_registry_wgsl`] avec `NoopRenderGraphV0Host`.
pub async fn run_graph_v0_checksum_with_registry_wgsl_host(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    doc: &RenderGraphDocument,
    readback_id: &str,
    registry: &RenderGraphGpuRegistry,
    pre_writes: &[(&str, u64, &[u8])],
    load_wgsl: &mut impl FnMut(&str) -> Result<String, RenderGraphExecError>,
    host: &mut impl RenderGraphV0Host,
) -> Result<u64, RenderGraphExecError> {
    validate_render_graph_exec_v0(doc, readback_id)?;

    for &(buffer_id, offset, data) in pre_writes {
        queue.write_buffer(registry.buffer(buffer_id)?, offset, data);
    }

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("render_graph_v0"),
    });

    encode_render_graph_passes_v0_with_wgsl_host(
        &mut encoder,
        device,
        registry,
        doc,
        load_wgsl,
        host,
    )?;

    run_graph_v0_readback_and_checksum(device, queue, readback_id, registry, encoder).await
}

/// Sans hooks hôte (compatible code existant).
pub async fn run_graph_v0_checksum_with_registry_wgsl(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    doc: &RenderGraphDocument,
    readback_id: &str,
    registry: &RenderGraphGpuRegistry,
    pre_writes: &[(&str, u64, &[u8])],
    load_wgsl: &mut impl FnMut(&str) -> Result<String, RenderGraphExecError>,
) -> Result<u64, RenderGraphExecError> {
    let mut n = NoopRenderGraphV0Host;
    run_graph_v0_checksum_with_registry_wgsl_host(
        device,
        queue,
        doc,
        readback_id,
        registry,
        pre_writes,
        load_wgsl,
        &mut n,
    )
    .await
}

/// Allocate resources + run checksum (no `shader_root` on disk) — e.g. browser fetches JSON + WGSL.
pub async fn run_graph_v0_checksum_from_wgsl(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    doc: &RenderGraphDocument,
    readback_id: &str,
    pre_writes: &[(&str, u64, &[u8])],
    load_wgsl: &mut impl FnMut(&str) -> Result<String, RenderGraphExecError>,
) -> Result<u64, RenderGraphExecError> {
    let registry = RenderGraphGpuRegistry::new(device, doc)?;
    run_graph_v0_checksum_with_registry_wgsl(
        device,
        queue,
        doc,
        readback_id,
        &registry,
        pre_writes,
        load_wgsl,
    )
    .await
}

#[cfg(not(target_arch = "wasm32"))]
/// Same as [`run_graph_v0_checksum_with_registry_wgsl`]: loads WGSL from `shader_root` on disk, with
/// optional **`queue.write_buffer`** (e.g. `dispatch_workgroups_indirect` seed for tests).
pub fn run_graph_v0_checksum_with_registry_pre_writes(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    doc: &RenderGraphDocument,
    shader_root: &Path,
    readback_id: &str,
    registry: &RenderGraphGpuRegistry,
    pre_writes: &[(&str, u64, &[u8])],
) -> Result<u64, RenderGraphExecError> {
    let mut load = |rel: &str| {
        std::fs::read_to_string(shader_root.join(rel)).map_err(RenderGraphExecError::from)
    };
    block_on(run_graph_v0_checksum_with_registry_wgsl(
        device,
        queue,
        doc,
        readback_id,
        registry,
        pre_writes,
        &mut load,
    ))
}

async fn run_graph_v0_readback_and_checksum(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    readback_id: &str,
    registry: &RenderGraphGpuRegistry,
    mut encoder: wgpu::CommandEncoder,
) -> Result<u64, RenderGraphExecError> {
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
    // Soumet la copie sur la file (natif) ; côté Web, `poll` n’y fait pas le mapping — voir le .await
    // ci-dessous.
    device.poll(wgpu::Maintain::Wait);

    let (tx, rx) = oneshot::channel();
    readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Natif: le callback s’exécute pendant le poll, avant le await.
        device.poll(wgpu::Maintain::Wait);
    }
    let map_res = rx
        .await
        .map_err(|_| RenderGraphExecError::ReadbackMapOneshotDropped)?;
    map_res.map_err(RenderGraphExecError::ReadbackBufferMapFailed)?;

    let data = readback.slice(..).get_mapped_range();
    let sum = fnv1a64(&data);
    drop(data);
    readback.unmap();
    Ok(sum)
}

/// Same as [`run_graph_v0_checksum`] but uses an existing [`RenderGraphGpuRegistry`] (e.g. after
/// [`RenderGraphGpuRegistry::resize_texture_2d`]) so GPU allocation can diverge from the
/// declared `width` / `height` in `doc` until the schema tracks runtime size.
#[cfg(not(target_arch = "wasm32"))]
pub fn run_graph_v0_checksum_with_registry(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    doc: &RenderGraphDocument,
    shader_root: &Path,
    readback_id: &str,
    registry: &RenderGraphGpuRegistry,
) -> Result<u64, RenderGraphExecError> {
    run_graph_v0_checksum_with_registry_pre_writes(
        device,
        queue,
        doc,
        shader_root,
        readback_id,
        registry,
        &[],
    )
}

/// Encode and submit one frame: all passes in document order, then return a **stable checksum**
/// of the `readback_id` texture (`Rgba16Float` only in v0).
#[cfg(not(target_arch = "wasm32"))]
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
