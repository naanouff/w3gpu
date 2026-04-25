//! Static checks for **v0 GPU execution** (formats, usages, pass wiring, readback target).
//! No `wgpu` — safe on **native and WASM** (same rules as `w3drs_renderer::render_graph_exec`).
//!
//! Phase **B.2** (subset): **semantic usage** per pass (declared `usage[]` on resources must
//! cover how passes reference textures) + duplicate color attachments + depth-aspect formats.

use crate::{Pass, RenderGraphDocument, Resource};

#[derive(Debug, thiserror::Error)]
pub enum RenderGraphValidateError {
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
    #[error("pass {pass_id:?} must omit `{field}` or set a non-empty string (B.6 ECS nœud)")]
    EcsLabelEmpty {
        pass_id: String,
        field: &'static str,
    },
    #[error("raster_depth_mesh pass {pass_id:?}: {detail}")]
    RasterDepthMeshInvalid { pass_id: String, detail: String },
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

fn texture_format_known(s: &str) -> Result<(), RenderGraphValidateError> {
    match s {
        "Rgba16Float" | "Rgba8Unorm" => Ok(()),
        "Depth24Plus" | "Depth32Float" | "Depth24PlusStencil8" | "Depth32FloatStencil8" => Ok(()),
        _ => Err(RenderGraphValidateError::UnknownTextureFormat(
            s.to_string(),
        )),
    }
}

fn is_depth_stencil_format(s: &str) -> bool {
    matches!(
        s,
        "Depth24Plus" | "Depth32Float" | "Depth24PlusStencil8" | "Depth32FloatStencil8"
    )
}

fn usage_has(usage: &[String], flag: &str) -> bool {
    usage.iter().any(|u| u == flag)
}

fn texture_resource<'a>(doc: &'a RenderGraphDocument, id: &str) -> Option<&'a Resource> {
    doc.resources.iter().find(|r| match r {
        Resource::Texture2d { id: tid, .. } => tid == id,
        Resource::Buffer { .. } => false,
    })
}

fn buffer_resource<'a>(doc: &'a RenderGraphDocument, id: &str) -> Option<&'a Resource> {
    doc.resources.iter().find(|r| match r {
        Resource::Buffer { id: bid, .. } => bid == id,
        Resource::Texture2d { .. } => false,
    })
}

/// LightUniforms in `shadow_depth` / moteur — 80 bytes (v0 exige au moins cette taille sur le buffer déclaré).
const RASTER_DEPTH_LIGHT_UNIFORM_MIN: u64 = 80;
/// Au moins une matrice instance 4×4 32-bit.
const RASTER_DEPTH_INSTANCE_MIN: u64 = 64;

fn check_ecs_string_options(
    pass_id: &str,
    ecs_before: &Option<String>,
    ecs_after: &Option<String>,
) -> Result<(), RenderGraphValidateError> {
    if ecs_before.as_ref().is_some_and(|s| s.is_empty()) {
        return Err(RenderGraphValidateError::EcsLabelEmpty {
            pass_id: pass_id.to_string(),
            field: "ecs_before",
        });
    }
    if ecs_after.as_ref().is_some_and(|s| s.is_empty()) {
        return Err(RenderGraphValidateError::EcsLabelEmpty {
            pass_id: pass_id.to_string(),
            field: "ecs_after",
        });
    }
    Ok(())
}

/// Ordered pass ids (submission order) — input to a future explicit barrier planner.
pub fn pass_ids_in_order_v0(doc: &RenderGraphDocument) -> Vec<&str> {
    doc.passes
        .iter()
        .map(|p| match p {
            Pass::Compute { id, .. }
            | Pass::RasterMesh { id, .. }
            | Pass::Fullscreen { id, .. }
            | Pass::Blit { id, .. }
            | Pass::RasterDepthMesh { id, .. } => id.as_str(),
        })
        .collect()
}

fn validate_raster_like_attachments_v0(
    doc: &RenderGraphDocument,
    pass_id: &str,
    color_targets: &[String],
    depth_target: &Option<String>,
    texture_ids: &std::collections::HashSet<String>,
) -> Result<(), RenderGraphValidateError> {
    let mut seen_ct = std::collections::HashSet::<&str>::new();
    for ct in color_targets {
        if !seen_ct.insert(ct.as_str()) {
            return Err(RenderGraphValidateError::DuplicateRasterColorTarget {
                pass_id: pass_id.to_string(),
                texture_id: ct.clone(),
            });
        }
        if !texture_ids.contains(ct) {
            return Err(RenderGraphValidateError::MissingResource(ct.clone()));
        }
        let Some(Resource::Texture2d { usage, .. }) = texture_resource(doc, ct) else {
            continue;
        };
        if !usage_has(usage, "render_attachment") {
            return Err(RenderGraphValidateError::TextureSemanticUsageMissing {
                pass_id: pass_id.to_string(),
                texture_id: ct.clone(),
                missing: "render_attachment".into(),
                declared: usage.clone(),
            });
        }
    }
    if let Some(dt) = depth_target {
        if !texture_ids.contains(dt) {
            return Err(RenderGraphValidateError::MissingResource(dt.clone()));
        }
        if let Some(Resource::Texture2d { format, usage, .. }) = texture_resource(doc, dt) {
            if !is_depth_stencil_format(format.as_str()) {
                return Err(RenderGraphValidateError::InvalidDepthTargetFormat {
                    texture_id: dt.clone(),
                    got: format.clone(),
                });
            }
            if !usage_has(usage, "render_attachment") {
                return Err(RenderGraphValidateError::TextureSemanticUsageMissing {
                    pass_id: pass_id.to_string(),
                    texture_id: dt.clone(),
                    missing: "render_attachment".into(),
                    declared: usage.clone(),
                });
            }
        }
    }
    Ok(())
}

fn validate_pass_resource_semantics_v0(
    doc: &RenderGraphDocument,
) -> Result<(), RenderGraphValidateError> {
    let mut texture_ids = std::collections::HashSet::<String>::new();
    let mut buffer_ids = std::collections::HashSet::<String>::new();
    for r in &doc.resources {
        match r {
            Resource::Texture2d { id, .. } => {
                texture_ids.insert(id.clone());
            }
            Resource::Buffer { id, .. } => {
                buffer_ids.insert(id.clone());
            }
        }
    }

    for p in &doc.passes {
        {
            let (pass_id, ecs_before, ecs_after) = match p {
                Pass::Compute {
                    id,
                    ecs_before,
                    ecs_after,
                    ..
                }
                | Pass::RasterMesh {
                    id,
                    ecs_before,
                    ecs_after,
                    ..
                }
                | Pass::Fullscreen {
                    id,
                    ecs_before,
                    ecs_after,
                    ..
                }
                | Pass::Blit {
                    id,
                    ecs_before,
                    ecs_after,
                    ..
                }
                | Pass::RasterDepthMesh {
                    id,
                    ecs_before,
                    ecs_after,
                    ..
                } => (id, ecs_before, ecs_after),
            };
            check_ecs_string_options(pass_id, ecs_before, ecs_after)?;
        }
        match p {
            Pass::Compute {
                id: pass_id,
                texture_reads,
                storage_writes,
                storage_buffers,
                storage_buffers_read,
                storage_buffers_group1,
                storage_buffers_read_group1,
                indirect_dispatch,
                ..
            } => {
                let mut seen_st = std::collections::HashSet::<&str>::new();
                for tid in storage_writes {
                    if !seen_st.insert(tid.as_str()) {
                        return Err(RenderGraphValidateError::DuplicateComputeStorageTexture {
                            pass_id: pass_id.clone(),
                            texture_id: tid.clone(),
                        });
                    }
                    if !texture_ids.contains(tid) {
                        return Err(RenderGraphValidateError::MissingResource(tid.clone()));
                    }
                    let Some(Resource::Texture2d { usage, .. }) = texture_resource(doc, tid) else {
                        continue;
                    };
                    if !usage_has(usage, "storage") {
                        return Err(RenderGraphValidateError::TextureSemanticUsageMissing {
                            pass_id: pass_id.clone(),
                            texture_id: tid.clone(),
                            missing: "storage".into(),
                            declared: usage.clone(),
                        });
                    }
                }
                let mut seen_tr = std::collections::HashSet::<&str>::new();
                for tid in texture_reads {
                    if !seen_tr.insert(tid.as_str()) {
                        return Err(RenderGraphValidateError::DuplicateComputeReadTexture {
                            pass_id: pass_id.clone(),
                            texture_id: tid.clone(),
                        });
                    }
                    if seen_st.contains(tid.as_str()) {
                        return Err(RenderGraphValidateError::ComputeTextureReadWriteConflict {
                            pass_id: pass_id.clone(),
                            texture_id: tid.clone(),
                        });
                    }
                    if !texture_ids.contains(tid) {
                        return Err(RenderGraphValidateError::MissingResource(tid.clone()));
                    }
                    let Some(Resource::Texture2d { format, usage, .. }) =
                        texture_resource(doc, tid)
                    else {
                        continue;
                    };
                    if is_depth_stencil_format(format.as_str()) {
                        return Err(RenderGraphValidateError::InvalidComputeTextureReadFormat {
                            pass_id: pass_id.clone(),
                            texture_id: tid.clone(),
                            got: format.clone(),
                        });
                    }
                    if format != "Rgba16Float" && format != "Rgba8Unorm" {
                        return Err(RenderGraphValidateError::InvalidComputeTextureReadFormat {
                            pass_id: pass_id.clone(),
                            texture_id: tid.clone(),
                            got: format.clone(),
                        });
                    }
                    if !usage_has(usage, "texture_binding") {
                        return Err(RenderGraphValidateError::TextureSemanticUsageMissing {
                            pass_id: pass_id.clone(),
                            texture_id: tid.clone(),
                            missing: "texture_binding".into(),
                            declared: usage.clone(),
                        });
                    }
                }
                let mut seen_buf_rw = std::collections::HashSet::<&str>::new();
                for bid in storage_buffers {
                    if !seen_buf_rw.insert(bid.as_str()) {
                        return Err(RenderGraphValidateError::DuplicateComputeStorageBuffer {
                            pass_id: pass_id.clone(),
                            buffer_id: bid.clone(),
                        });
                    }
                    if !buffer_ids.contains(bid) {
                        return Err(RenderGraphValidateError::MissingResource(bid.clone()));
                    }
                    let Some(Resource::Buffer { usage, .. }) = buffer_resource(doc, bid) else {
                        continue;
                    };
                    if !usage_has(usage, "storage") {
                        return Err(RenderGraphValidateError::BufferSemanticUsageMissing {
                            pass_id: pass_id.clone(),
                            buffer_id: bid.clone(),
                            missing: "storage".into(),
                            declared: usage.clone(),
                        });
                    }
                }
                let mut seen_buf_ro = std::collections::HashSet::<&str>::new();
                for bid in storage_buffers_read {
                    if !seen_buf_ro.insert(bid.as_str()) {
                        return Err(
                            RenderGraphValidateError::DuplicateComputeStorageBufferRead {
                                pass_id: pass_id.clone(),
                                buffer_id: bid.clone(),
                            },
                        );
                    }
                    if seen_buf_rw.contains(bid.as_str()) {
                        return Err(RenderGraphValidateError::ComputeStorageBufferRwRoConflict {
                            pass_id: pass_id.clone(),
                            buffer_id: bid.clone(),
                        });
                    }
                    if !buffer_ids.contains(bid) {
                        return Err(RenderGraphValidateError::MissingResource(bid.clone()));
                    }
                    let Some(Resource::Buffer { usage, .. }) = buffer_resource(doc, bid) else {
                        continue;
                    };
                    if !usage_has(usage, "storage") {
                        return Err(RenderGraphValidateError::BufferSemanticUsageMissing {
                            pass_id: pass_id.clone(),
                            buffer_id: bid.clone(),
                            missing: "storage".into(),
                            declared: usage.clone(),
                        });
                    }
                }
                let g0_binding_count = storage_buffers.len()
                    + storage_buffers_read.len()
                    + storage_writes.len()
                    + texture_reads.len();
                let g1_has_buffers =
                    !storage_buffers_group1.is_empty() || !storage_buffers_read_group1.is_empty();
                if g1_has_buffers && g0_binding_count == 0 {
                    return Err(RenderGraphValidateError::ComputeGroup1RequiresGroup0 {
                        pass_id: pass_id.clone(),
                    });
                }
                let mut seen_g1_rw = std::collections::HashSet::<&str>::new();
                for bid in storage_buffers_group1 {
                    if !seen_g1_rw.insert(bid.as_str()) {
                        return Err(
                            RenderGraphValidateError::DuplicateComputeStorageBufferGroup1 {
                                pass_id: pass_id.clone(),
                                buffer_id: bid.clone(),
                            },
                        );
                    }
                    if seen_buf_rw.contains(bid.as_str()) || seen_buf_ro.contains(bid.as_str()) {
                        return Err(
                            RenderGraphValidateError::ComputeBufferSharedAcrossBindGroups {
                                pass_id: pass_id.clone(),
                                buffer_id: bid.clone(),
                            },
                        );
                    }
                    if !buffer_ids.contains(bid) {
                        return Err(RenderGraphValidateError::MissingResource(bid.clone()));
                    }
                    let Some(Resource::Buffer { usage, .. }) = buffer_resource(doc, bid) else {
                        continue;
                    };
                    if !usage_has(usage, "storage") {
                        return Err(RenderGraphValidateError::BufferSemanticUsageMissing {
                            pass_id: pass_id.clone(),
                            buffer_id: bid.clone(),
                            missing: "storage".into(),
                            declared: usage.clone(),
                        });
                    }
                }
                let mut seen_g1_ro = std::collections::HashSet::<&str>::new();
                for bid in storage_buffers_read_group1 {
                    if !seen_g1_ro.insert(bid.as_str()) {
                        return Err(
                            RenderGraphValidateError::DuplicateComputeStorageBufferReadGroup1 {
                                pass_id: pass_id.clone(),
                                buffer_id: bid.clone(),
                            },
                        );
                    }
                    if seen_g1_rw.contains(bid.as_str()) {
                        return Err(
                            RenderGraphValidateError::ComputeStorageBufferGroup1RwRoConflict {
                                pass_id: pass_id.clone(),
                                buffer_id: bid.clone(),
                            },
                        );
                    }
                    if seen_buf_rw.contains(bid.as_str()) || seen_buf_ro.contains(bid.as_str()) {
                        return Err(
                            RenderGraphValidateError::ComputeBufferSharedAcrossBindGroups {
                                pass_id: pass_id.clone(),
                                buffer_id: bid.clone(),
                            },
                        );
                    }
                    if !buffer_ids.contains(bid) {
                        return Err(RenderGraphValidateError::MissingResource(bid.clone()));
                    }
                    let Some(Resource::Buffer { usage, .. }) = buffer_resource(doc, bid) else {
                        continue;
                    };
                    if !usage_has(usage, "storage") {
                        return Err(RenderGraphValidateError::BufferSemanticUsageMissing {
                            pass_id: pass_id.clone(),
                            buffer_id: bid.clone(),
                            missing: "storage".into(),
                            declared: usage.clone(),
                        });
                    }
                }
                if let Some(ind) = indirect_dispatch {
                    if ind.offset % 4 != 0 {
                        return Err(RenderGraphValidateError::IndirectDispatchOffsetMisaligned {
                            pass_id: pass_id.clone(),
                            buffer_id: ind.buffer.clone(),
                            offset: ind.offset,
                        });
                    }
                    if !buffer_ids.contains(&ind.buffer) {
                        return Err(RenderGraphValidateError::MissingResource(
                            ind.buffer.clone(),
                        ));
                    }
                    let Some(Resource::Buffer { size, usage, .. }) =
                        buffer_resource(doc, &ind.buffer)
                    else {
                        return Err(RenderGraphValidateError::MissingResource(
                            ind.buffer.clone(),
                        ));
                    };
                    if ind.offset.saturating_add(12) > *size {
                        return Err(RenderGraphValidateError::IndirectDispatchOutOfBuffer {
                            pass_id: pass_id.clone(),
                            buffer_id: ind.buffer.clone(),
                            offset: ind.offset,
                            size: *size,
                        });
                    }
                    if !usage_has(usage, "indirect") {
                        return Err(RenderGraphValidateError::IndirectBufferUsageMissing {
                            pass_id: pass_id.clone(),
                            buffer_id: ind.buffer.clone(),
                            missing: "indirect".into(),
                            declared: usage.clone(),
                        });
                    }
                }
            }
            Pass::RasterMesh {
                id: pass_id,
                color_targets,
                depth_target,
                ..
            } => {
                validate_raster_like_attachments_v0(
                    doc,
                    pass_id,
                    color_targets,
                    depth_target,
                    &texture_ids,
                )?;
            }
            Pass::Fullscreen {
                id: pass_id,
                color_targets,
                depth_target,
                ..
            } => {
                validate_raster_like_attachments_v0(
                    doc,
                    pass_id,
                    color_targets,
                    depth_target,
                    &texture_ids,
                )?;
            }
            Pass::Blit {
                id: pass_id,
                source,
                destination,
                region,
                ..
            } => {
                if source == destination {
                    return Err(RenderGraphValidateError::BlitSameTexture {
                        pass_id: pass_id.clone(),
                        texture_id: source.clone(),
                    });
                }
                if !texture_ids.contains(source) {
                    return Err(RenderGraphValidateError::MissingResource(source.clone()));
                }
                if !texture_ids.contains(destination) {
                    return Err(RenderGraphValidateError::MissingResource(
                        destination.clone(),
                    ));
                }
                let Some(Resource::Texture2d {
                    format: src_format,
                    width: src_w,
                    height: src_h,
                    usage: src_usage,
                    mip_level_count: src_mips,
                    ..
                }) = texture_resource(doc, source)
                else {
                    return Err(RenderGraphValidateError::MissingResource(source.clone()));
                };
                let Some(Resource::Texture2d {
                    format: dst_format,
                    width: dst_w,
                    height: dst_h,
                    usage: dst_usage,
                    mip_level_count: dst_mips,
                    ..
                }) = texture_resource(doc, destination)
                else {
                    return Err(RenderGraphValidateError::MissingResource(
                        destination.clone(),
                    ));
                };
                let src_fmt = src_format.as_str();
                let dst_fmt = dst_format.as_str();
                if src_fmt != dst_fmt {
                    return Err(RenderGraphValidateError::BlitFormatMismatch {
                        pass_id: pass_id.clone(),
                        src_id: source.clone(),
                        dst_id: destination.clone(),
                        src_fmt: src_format.clone(),
                        dst_fmt: dst_format.clone(),
                    });
                }
                if region.is_none() && (*src_w != *dst_w || *src_h != *dst_h) {
                    return Err(RenderGraphValidateError::BlitExtentMismatch {
                        pass_id: pass_id.clone(),
                        src_id: source.clone(),
                        dst_id: destination.clone(),
                        sw: *src_w,
                        sh: *src_h,
                        dw: *dst_w,
                        dh: *dst_h,
                    });
                }
                if let Some(reg) = region {
                    if reg.src_mip_level >= *src_mips || reg.dst_mip_level >= *dst_mips {
                        return Err(RenderGraphValidateError::BlitRegionInvalid {
                            pass_id: pass_id.clone(),
                            src_id: source.clone(),
                            dst_id: destination.clone(),
                        });
                    }
                    let (sw, sh) = mip_dimensions(*src_w, *src_h, reg.src_mip_level);
                    let (dw, dh) = mip_dimensions(*dst_w, *dst_h, reg.dst_mip_level);
                    if reg.src_origin_x >= sw
                        || reg.src_origin_y >= sh
                        || reg.dst_origin_x >= dw
                        || reg.dst_origin_y >= dh
                    {
                        return Err(RenderGraphValidateError::BlitRegionInvalid {
                            pass_id: pass_id.clone(),
                            src_id: source.clone(),
                            dst_id: destination.clone(),
                        });
                    }
                    let max_w = (sw - reg.src_origin_x).min(dw - reg.dst_origin_x);
                    let max_h = (sh - reg.src_origin_y).min(dh - reg.dst_origin_y);
                    let copy_w = reg.width.unwrap_or(max_w).min(max_w);
                    let copy_h = reg.height.unwrap_or(max_h).min(max_h);
                    if copy_w == 0 || copy_h == 0 {
                        return Err(RenderGraphValidateError::BlitRegionInvalid {
                            pass_id: pass_id.clone(),
                            src_id: source.clone(),
                            dst_id: destination.clone(),
                        });
                    }
                }
                if !usage_has(src_usage, "copy_src") {
                    return Err(RenderGraphValidateError::TextureSemanticUsageMissing {
                        pass_id: pass_id.clone(),
                        texture_id: source.clone(),
                        missing: "copy_src".into(),
                        declared: src_usage.clone(),
                    });
                }
                if !usage_has(dst_usage, "copy_dst") {
                    return Err(RenderGraphValidateError::TextureSemanticUsageMissing {
                        pass_id: pass_id.clone(),
                        texture_id: destination.clone(),
                        missing: "copy_dst".into(),
                        declared: dst_usage.clone(),
                    });
                }
            }
            Pass::RasterDepthMesh {
                id: pass_id,
                depth_target,
                light_uniforms_buffer,
                instance_buffer,
                ..
            } => {
                if !texture_ids.contains(depth_target) {
                    return Err(RenderGraphValidateError::MissingResource(
                        depth_target.clone(),
                    ));
                }
                if !buffer_ids.contains(light_uniforms_buffer) {
                    return Err(RenderGraphValidateError::MissingResource(
                        light_uniforms_buffer.clone(),
                    ));
                }
                if !buffer_ids.contains(instance_buffer) {
                    return Err(RenderGraphValidateError::MissingResource(
                        instance_buffer.clone(),
                    ));
                }
                if light_uniforms_buffer == instance_buffer {
                    return Err(RenderGraphValidateError::RasterDepthMeshInvalid {
                        pass_id: pass_id.clone(),
                        detail: "light_uniforms_buffer and instance_buffer must differ".into(),
                    });
                }
                if let Some(Resource::Texture2d { format, usage, .. }) =
                    texture_resource(doc, depth_target)
                {
                    if !is_depth_stencil_format(format.as_str()) {
                        return Err(RenderGraphValidateError::InvalidDepthTargetFormat {
                            texture_id: depth_target.clone(),
                            got: format.clone(),
                        });
                    }
                    if !usage_has(usage, "render_attachment") {
                        return Err(RenderGraphValidateError::TextureSemanticUsageMissing {
                            pass_id: pass_id.clone(),
                            texture_id: depth_target.clone(),
                            missing: "render_attachment".into(),
                            declared: usage.clone(),
                        });
                    }
                }
                if let Some(Resource::Buffer { size, usage, .. }) =
                    buffer_resource(doc, light_uniforms_buffer)
                {
                    if *size < RASTER_DEPTH_LIGHT_UNIFORM_MIN {
                        return Err(RenderGraphValidateError::RasterDepthMeshInvalid {
                            pass_id: pass_id.clone(),
                            detail: format!(
                                "light_uniforms_buffer {light_uniforms_buffer:?} size {size} < {RASTER_DEPTH_LIGHT_UNIFORM_MIN} (LightUniforms)"
                            ),
                        });
                    }
                    if !usage_has(usage, "uniform") {
                        return Err(RenderGraphValidateError::BufferSemanticUsageMissing {
                            pass_id: pass_id.clone(),
                            buffer_id: light_uniforms_buffer.clone(),
                            missing: "uniform".into(),
                            declared: usage.clone(),
                        });
                    }
                }
                if let Some(Resource::Buffer { size, usage, .. }) =
                    buffer_resource(doc, instance_buffer)
                {
                    if *size < RASTER_DEPTH_INSTANCE_MIN {
                        return Err(RenderGraphValidateError::RasterDepthMeshInvalid {
                            pass_id: pass_id.clone(),
                            detail: format!(
                                "instance_buffer {instance_buffer:?} size {size} < {RASTER_DEPTH_INSTANCE_MIN}"
                            ),
                        });
                    }
                    if !usage_has(usage, "storage") {
                        return Err(RenderGraphValidateError::BufferSemanticUsageMissing {
                            pass_id: pass_id.clone(),
                            buffer_id: instance_buffer.clone(),
                            missing: "storage".into(),
                            declared: usage.clone(),
                        });
                    }
                }
            }
        }
    }

    Ok(())
}

fn buffer_usage_known(flags: &[String]) -> Result<(), RenderGraphValidateError> {
    for s in flags {
        match s.as_str() {
            "storage" | "copy_dst" | "copy_src" | "map_read" | "indirect" | "uniform" => {}
            other => {
                return Err(RenderGraphValidateError::UnknownBufferUsage(
                    other.to_string(),
                ));
            }
        }
    }
    Ok(())
}

fn texture_usage_known(flags: &[String]) -> Result<(), RenderGraphValidateError> {
    for s in flags {
        match s.as_str() {
            "texture_binding" | "render_attachment" | "storage" | "copy_dst" | "copy_src" => {}
            other => {
                return Err(RenderGraphValidateError::UnknownTextureUsage(
                    other.to_string(),
                ));
            }
        }
    }
    Ok(())
}

/// Same rules as `w3drs_renderer::validate_render_graph_exec_v0` before GPU submit.
pub fn validate_exec_v0(
    doc: &RenderGraphDocument,
    readback_id: &str,
) -> Result<(), RenderGraphValidateError> {
    let mut texture_ids = std::collections::HashSet::<String>::new();
    for r in &doc.resources {
        match r {
            Resource::Texture2d {
                id,
                format,
                usage,
                mip_level_count,
                ..
            } => {
                texture_format_known(format)?;
                texture_usage_known(usage)?;
                if *mip_level_count == 0 || *mip_level_count > 32 {
                    return Err(RenderGraphValidateError::InvalidTextureMipLevelCount {
                        id: id.clone(),
                        got: *mip_level_count,
                    });
                }
                texture_ids.insert(id.clone());
            }
            Resource::Buffer { usage, .. } => {
                buffer_usage_known(usage)?;
            }
        }
    }

    if !texture_ids.contains(readback_id) {
        return Err(RenderGraphValidateError::MissingResource(
            readback_id.to_string(),
        ));
    }
    if let Some(Resource::Texture2d { format, .. }) = doc
        .resources
        .iter()
        .find(|r| matches!(r, Resource::Texture2d { id, .. } if id == readback_id))
    {
        if format != "Rgba16Float" {
            return Err(RenderGraphValidateError::InvalidReadbackFormat {
                id: readback_id.to_string(),
                got: format.clone(),
            });
        }
    }

    for p in &doc.passes {
        match p {
            Pass::RasterMesh {
                id, color_targets, ..
            }
            | Pass::Fullscreen {
                id, color_targets, ..
            } => {
                if color_targets.is_empty() {
                    return Err(RenderGraphValidateError::EmptyColorTargets(id.clone()));
                }
                for ct in color_targets {
                    if !texture_ids.contains(ct) {
                        return Err(RenderGraphValidateError::MissingResource(ct.clone()));
                    }
                }
            }
            _ => {}
        }
    }

    validate_pass_resource_semantics_v0(doc)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Dispatch3D, Pass, RenderGraphDocument, Resource};

    fn doc_minimal_tex(
        format: &str,
        tex_usage: Vec<String>,
        buf_usage: Vec<String>,
    ) -> RenderGraphDocument {
        RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![
                Resource::Texture2d {
                    id: "hdr_color".into(),
                    format: format.into(),
                    width: 4,
                    height: 4,
                    usage: tex_usage,
                    mip_level_count: 1,
                },
                Resource::Buffer {
                    id: "b".into(),
                    size: 16,
                    usage: buf_usage,
                },
            ],
            passes: vec![
                Pass::Compute {
                    ecs_before: None,
                    ecs_after: None,
                    id: "c".into(),
                    shader: "shaders/x.wgsl".into(),
                    entry_point: "cs_main".into(),
                    dispatch: Dispatch3D { x: 1, y: 1, z: 1 },
                    texture_reads: vec![],
                    storage_writes: vec![],
                    storage_buffers: vec![],
                    storage_buffers_read: vec![],
                    storage_buffers_group1: vec![],
                    storage_buffers_read_group1: vec![],
                    indirect_dispatch: None,
                },
                Pass::RasterMesh {
                    ecs_before: None,
                    ecs_after: None,
                    id: "r".into(),
                    shader: "shaders/y.wgsl".into(),
                    vertex_entry: "vs_main".into(),
                    fragment_entry: "fs_main".into(),
                    color_targets: vec!["hdr_color".into()],
                    depth_target: None,
                },
            ],
        }
    }

    #[test]
    fn validate_rejects_unknown_texture_format() {
        let doc = doc_minimal_tex("Bc1RgbaUnorm", vec![], vec!["storage".into()]);
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::UnknownTextureFormat(ref s) if s == "Bc1RgbaUnorm"
        ));
    }

    #[test]
    fn validate_rejects_unknown_texture_usage_flag() {
        let doc = doc_minimal_tex(
            "Rgba16Float",
            vec!["not_a_real_usage".into()],
            vec!["storage".into()],
        );
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::UnknownTextureUsage(ref s) if s == "not_a_real_usage"
        ));
    }

    #[test]
    fn validate_rejects_unknown_buffer_usage_flag() {
        let doc = doc_minimal_tex(
            "Rgba16Float",
            vec!["render_attachment".into()],
            vec!["bad_buf_usage".into()],
        );
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::UnknownBufferUsage(ref s) if s == "bad_buf_usage"
        ));
    }

    #[test]
    fn validate_rejects_readback_when_tex_is_rgba8_unorm() {
        let doc = doc_minimal_tex(
            "Rgba8Unorm",
            vec!["render_attachment".into()],
            vec!["storage".into()],
        );
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::InvalidReadbackFormat { ref id, ref got }
                if id == "hdr_color" && got == "Rgba8Unorm"
        ));
    }

    #[test]
    fn validate_rejects_missing_readback() {
        let doc = doc_minimal_tex("Rgba16Float", vec![], vec!["storage".into()]);
        let e = validate_exec_v0(&doc, "nope").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::MissingResource(ref s) if s == "nope"
        ));
    }

    #[test]
    fn validate_rejects_empty_color_targets() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![Resource::Texture2d {
                id: "hdr_color".into(),
                format: "Rgba16Float".into(),
                width: 4,
                height: 4,
                usage: vec!["render_attachment".into()],
                mip_level_count: 1,
            }],
            passes: vec![Pass::RasterMesh {
                ecs_before: None,
                ecs_after: None,
                id: "r".into(),
                shader: "s.wgsl".into(),
                vertex_entry: "vs_main".into(),
                fragment_entry: "fs_main".into(),
                color_targets: vec![],
                depth_target: None,
            }],
        };
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::EmptyColorTargets(ref s) if s == "r"
        ));
    }

    #[test]
    fn validate_rejects_raster_color_target_unknown() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![Resource::Texture2d {
                id: "hdr_color".into(),
                format: "Rgba16Float".into(),
                width: 4,
                height: 4,
                usage: vec!["render_attachment".into()],
                mip_level_count: 1,
            }],
            passes: vec![Pass::RasterMesh {
                ecs_before: None,
                ecs_after: None,
                id: "r".into(),
                shader: "s.wgsl".into(),
                vertex_entry: "vs_main".into(),
                fragment_entry: "fs_main".into(),
                color_targets: vec!["other_tex".into()],
                depth_target: None,
            }],
        };
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::MissingResource(ref s) if s == "other_tex"
        ));
    }

    #[test]
    fn validate_accepts_fixture_shape() {
        let doc = doc_minimal_tex(
            "Rgba16Float",
            vec!["texture_binding".into(), "render_attachment".into()],
            vec!["storage".into(), "copy_dst".into()],
        );
        validate_exec_v0(&doc, "hdr_color").unwrap();
    }

    #[test]
    fn validate_rejects_raster_without_render_attachment_usage() {
        let doc = doc_minimal_tex(
            "Rgba16Float",
            vec!["texture_binding".into()],
            vec!["storage".into()],
        );
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::TextureSemanticUsageMissing {
                ref missing,
                ..
            } if missing == "render_attachment"
        ));
    }

    #[test]
    fn validate_rejects_duplicate_raster_color_targets() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![Resource::Texture2d {
                id: "hdr_color".into(),
                format: "Rgba16Float".into(),
                width: 4,
                height: 4,
                usage: vec!["render_attachment".into()],
                mip_level_count: 1,
            }],
            passes: vec![Pass::RasterMesh {
                ecs_before: None,
                ecs_after: None,
                id: "r".into(),
                shader: "s.wgsl".into(),
                vertex_entry: "vs_main".into(),
                fragment_entry: "fs_main".into(),
                color_targets: vec!["hdr_color".into(), "hdr_color".into()],
                depth_target: None,
            }],
        };
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::DuplicateRasterColorTarget { .. }
        ));
    }

    #[test]
    fn validate_rejects_depth_target_non_depth_format() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![Resource::Texture2d {
                id: "hdr_color".into(),
                format: "Rgba16Float".into(),
                width: 4,
                height: 4,
                usage: vec!["render_attachment".into()],
                mip_level_count: 1,
            }],
            passes: vec![Pass::RasterMesh {
                ecs_before: None,
                ecs_after: None,
                id: "r".into(),
                shader: "s.wgsl".into(),
                vertex_entry: "vs_main".into(),
                fragment_entry: "fs_main".into(),
                color_targets: vec!["hdr_color".into()],
                depth_target: Some("hdr_color".into()),
            }],
        };
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::InvalidDepthTargetFormat { .. }
        ));
    }

    #[test]
    fn validate_rejects_compute_storage_write_without_storage_usage() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![
                Resource::Texture2d {
                    id: "hdr_color".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["render_attachment".into(), "texture_binding".into()],
                    mip_level_count: 1,
                },
                Resource::Texture2d {
                    id: "work".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["texture_binding".into()],
                    mip_level_count: 1,
                },
            ],
            passes: vec![
                Pass::Compute {
                    ecs_before: None,
                    ecs_after: None,
                    id: "c".into(),
                    shader: "x.wgsl".into(),
                    entry_point: "m".into(),
                    dispatch: Dispatch3D { x: 1, y: 1, z: 1 },
                    texture_reads: vec![],
                    storage_writes: vec!["work".into()],
                    storage_buffers: vec![],
                    storage_buffers_read: vec![],
                    storage_buffers_group1: vec![],
                    storage_buffers_read_group1: vec![],
                    indirect_dispatch: None,
                },
                Pass::RasterMesh {
                    ecs_before: None,
                    ecs_after: None,
                    id: "r".into(),
                    shader: "y.wgsl".into(),
                    vertex_entry: "vs_main".into(),
                    fragment_entry: "fs_main".into(),
                    color_targets: vec!["hdr_color".into()],
                    depth_target: None,
                },
            ],
        };
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::TextureSemanticUsageMissing {
                ref missing,
                ..
            } if missing == "storage"
        ));
    }

    #[test]
    fn pass_ids_in_order_matches_document() {
        let doc = doc_minimal_tex(
            "Rgba16Float",
            vec!["render_attachment".into()],
            vec!["storage".into()],
        );
        assert_eq!(pass_ids_in_order_v0(&doc), vec!["c", "r"]);
    }

    #[test]
    fn validate_rejects_compute_storage_buffer_without_storage_usage() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![
                Resource::Texture2d {
                    id: "hdr_color".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["render_attachment".into()],
                    mip_level_count: 1,
                },
                Resource::Buffer {
                    id: "indirect_args".into(),
                    size: 64,
                    usage: vec!["copy_dst".into()],
                },
            ],
            passes: vec![
                Pass::Compute {
                    ecs_before: None,
                    ecs_after: None,
                    id: "c".into(),
                    shader: "x.wgsl".into(),
                    entry_point: "m".into(),
                    dispatch: Dispatch3D { x: 1, y: 1, z: 1 },
                    texture_reads: vec![],
                    storage_writes: vec![],
                    storage_buffers: vec!["indirect_args".into()],
                    storage_buffers_read: vec![],
                    storage_buffers_group1: vec![],
                    storage_buffers_read_group1: vec![],
                    indirect_dispatch: None,
                },
                Pass::RasterMesh {
                    ecs_before: None,
                    ecs_after: None,
                    id: "r".into(),
                    shader: "y.wgsl".into(),
                    vertex_entry: "vs_main".into(),
                    fragment_entry: "fs_main".into(),
                    color_targets: vec!["hdr_color".into()],
                    depth_target: None,
                },
            ],
        };
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::BufferSemanticUsageMissing {
                ref missing,
                ..
            } if missing == "storage"
        ));
    }

    #[test]
    fn validate_rejects_duplicate_compute_storage_buffers() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![
                Resource::Texture2d {
                    id: "hdr_color".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["render_attachment".into()],
                    mip_level_count: 1,
                },
                Resource::Buffer {
                    id: "indirect_args".into(),
                    size: 64,
                    usage: vec!["storage".into()],
                },
            ],
            passes: vec![
                Pass::Compute {
                    ecs_before: None,
                    ecs_after: None,
                    id: "c".into(),
                    shader: "x.wgsl".into(),
                    entry_point: "m".into(),
                    dispatch: Dispatch3D { x: 1, y: 1, z: 1 },
                    texture_reads: vec![],
                    storage_writes: vec![],
                    storage_buffers: vec!["indirect_args".into(), "indirect_args".into()],
                    storage_buffers_read: vec![],
                    storage_buffers_group1: vec![],
                    storage_buffers_read_group1: vec![],
                    indirect_dispatch: None,
                },
                Pass::RasterMesh {
                    ecs_before: None,
                    ecs_after: None,
                    id: "r".into(),
                    shader: "y.wgsl".into(),
                    vertex_entry: "vs_main".into(),
                    fragment_entry: "fs_main".into(),
                    color_targets: vec!["hdr_color".into()],
                    depth_target: None,
                },
            ],
        };
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::DuplicateComputeStorageBuffer { .. }
        ));
    }

    #[test]
    fn validate_rejects_duplicate_compute_storage_writes() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![
                Resource::Texture2d {
                    id: "hdr_color".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["render_attachment".into()],
                    mip_level_count: 1,
                },
                Resource::Texture2d {
                    id: "ping".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["storage".into()],
                    mip_level_count: 1,
                },
            ],
            passes: vec![
                Pass::Compute {
                    ecs_before: None,
                    ecs_after: None,
                    id: "c".into(),
                    shader: "x.wgsl".into(),
                    entry_point: "m".into(),
                    dispatch: Dispatch3D { x: 1, y: 1, z: 1 },
                    texture_reads: vec![],
                    storage_writes: vec!["ping".into(), "ping".into()],
                    storage_buffers: vec![],
                    storage_buffers_read: vec![],
                    storage_buffers_group1: vec![],
                    storage_buffers_read_group1: vec![],
                    indirect_dispatch: None,
                },
                Pass::RasterMesh {
                    ecs_before: None,
                    ecs_after: None,
                    id: "r".into(),
                    shader: "y.wgsl".into(),
                    vertex_entry: "vs_main".into(),
                    fragment_entry: "fs_main".into(),
                    color_targets: vec!["hdr_color".into()],
                    depth_target: None,
                },
            ],
        };
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::DuplicateComputeStorageTexture { .. }
        ));
    }

    #[test]
    fn validate_rejects_duplicate_compute_texture_reads() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![
                Resource::Texture2d {
                    id: "hdr_color".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["render_attachment".into(), "texture_binding".into()],
                    mip_level_count: 1,
                },
                Resource::Texture2d {
                    id: "ping".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["storage".into()],
                    mip_level_count: 1,
                },
            ],
            passes: vec![
                Pass::Compute {
                    ecs_before: None,
                    ecs_after: None,
                    id: "c".into(),
                    shader: "x.wgsl".into(),
                    entry_point: "m".into(),
                    dispatch: Dispatch3D { x: 1, y: 1, z: 1 },
                    texture_reads: vec!["hdr_color".into(), "hdr_color".into()],
                    storage_writes: vec!["ping".into()],
                    storage_buffers: vec![],
                    storage_buffers_read: vec![],
                    storage_buffers_group1: vec![],
                    storage_buffers_read_group1: vec![],
                    indirect_dispatch: None,
                },
                Pass::RasterMesh {
                    ecs_before: None,
                    ecs_after: None,
                    id: "r".into(),
                    shader: "y.wgsl".into(),
                    vertex_entry: "vs_main".into(),
                    fragment_entry: "fs_main".into(),
                    color_targets: vec!["hdr_color".into()],
                    depth_target: None,
                },
            ],
        };
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::DuplicateComputeReadTexture { .. }
        ));
    }

    #[test]
    fn validate_rejects_compute_same_texture_read_and_storage_write() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![
                Resource::Texture2d {
                    id: "hdr_color".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["render_attachment".into(), "texture_binding".into()],
                    mip_level_count: 1,
                },
                Resource::Texture2d {
                    id: "ping".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["storage".into(), "texture_binding".into()],
                    mip_level_count: 1,
                },
            ],
            passes: vec![
                Pass::Compute {
                    ecs_before: None,
                    ecs_after: None,
                    id: "c".into(),
                    shader: "x.wgsl".into(),
                    entry_point: "m".into(),
                    dispatch: Dispatch3D { x: 1, y: 1, z: 1 },
                    texture_reads: vec!["ping".into()],
                    storage_writes: vec!["ping".into()],
                    storage_buffers: vec![],
                    storage_buffers_read: vec![],
                    storage_buffers_group1: vec![],
                    storage_buffers_read_group1: vec![],
                    indirect_dispatch: None,
                },
                Pass::RasterMesh {
                    ecs_before: None,
                    ecs_after: None,
                    id: "r".into(),
                    shader: "y.wgsl".into(),
                    vertex_entry: "vs_main".into(),
                    fragment_entry: "fs_main".into(),
                    color_targets: vec!["hdr_color".into()],
                    depth_target: None,
                },
            ],
        };
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::ComputeTextureReadWriteConflict { .. }
        ));
    }

    #[test]
    fn validate_rejects_compute_texture_read_depth_format() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![
                Resource::Texture2d {
                    id: "hdr_color".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["render_attachment".into()],
                    mip_level_count: 1,
                },
                Resource::Texture2d {
                    id: "scene_depth".into(),
                    format: "Depth32Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["render_attachment".into(), "texture_binding".into()],
                    mip_level_count: 1,
                },
                Resource::Texture2d {
                    id: "ping".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["storage".into()],
                    mip_level_count: 1,
                },
            ],
            passes: vec![
                Pass::Compute {
                    ecs_before: None,
                    ecs_after: None,
                    id: "c".into(),
                    shader: "x.wgsl".into(),
                    entry_point: "m".into(),
                    dispatch: Dispatch3D { x: 1, y: 1, z: 1 },
                    texture_reads: vec!["scene_depth".into()],
                    storage_writes: vec!["ping".into()],
                    storage_buffers: vec![],
                    storage_buffers_read: vec![],
                    storage_buffers_group1: vec![],
                    storage_buffers_read_group1: vec![],
                    indirect_dispatch: None,
                },
                Pass::RasterMesh {
                    ecs_before: None,
                    ecs_after: None,
                    id: "r".into(),
                    shader: "y.wgsl".into(),
                    vertex_entry: "vs_main".into(),
                    fragment_entry: "fs_main".into(),
                    color_targets: vec!["hdr_color".into()],
                    depth_target: None,
                },
            ],
        };
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::InvalidComputeTextureReadFormat { .. }
        ));
    }

    #[test]
    fn validate_rejects_duplicate_compute_storage_buffers_read() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![
                Resource::Texture2d {
                    id: "hdr_color".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["render_attachment".into()],
                    mip_level_count: 1,
                },
                Resource::Buffer {
                    id: "ro_a".into(),
                    size: 64,
                    usage: vec!["storage".into()],
                },
            ],
            passes: vec![
                Pass::Compute {
                    ecs_before: None,
                    ecs_after: None,
                    id: "c".into(),
                    shader: "x.wgsl".into(),
                    entry_point: "m".into(),
                    dispatch: Dispatch3D { x: 1, y: 1, z: 1 },
                    texture_reads: vec![],
                    storage_writes: vec![],
                    storage_buffers: vec![],
                    storage_buffers_read: vec!["ro_a".into(), "ro_a".into()],
                    storage_buffers_group1: vec![],
                    storage_buffers_read_group1: vec![],
                    indirect_dispatch: None,
                },
                Pass::RasterMesh {
                    ecs_before: None,
                    ecs_after: None,
                    id: "r".into(),
                    shader: "y.wgsl".into(),
                    vertex_entry: "vs_main".into(),
                    fragment_entry: "fs_main".into(),
                    color_targets: vec!["hdr_color".into()],
                    depth_target: None,
                },
            ],
        };
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::DuplicateComputeStorageBufferRead { .. }
        ));
    }

    #[test]
    fn validate_rejects_compute_same_buffer_rw_and_ro_lists() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![
                Resource::Texture2d {
                    id: "hdr_color".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["render_attachment".into()],
                    mip_level_count: 1,
                },
                Resource::Buffer {
                    id: "b".into(),
                    size: 64,
                    usage: vec!["storage".into()],
                },
            ],
            passes: vec![
                Pass::Compute {
                    ecs_before: None,
                    ecs_after: None,
                    id: "c".into(),
                    shader: "x.wgsl".into(),
                    entry_point: "m".into(),
                    dispatch: Dispatch3D { x: 1, y: 1, z: 1 },
                    texture_reads: vec![],
                    storage_writes: vec![],
                    storage_buffers: vec!["b".into()],
                    storage_buffers_read: vec!["b".into()],
                    storage_buffers_group1: vec![],
                    storage_buffers_read_group1: vec![],
                    indirect_dispatch: None,
                },
                Pass::RasterMesh {
                    ecs_before: None,
                    ecs_after: None,
                    id: "r".into(),
                    shader: "y.wgsl".into(),
                    vertex_entry: "vs_main".into(),
                    fragment_entry: "fs_main".into(),
                    color_targets: vec!["hdr_color".into()],
                    depth_target: None,
                },
            ],
        };
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::ComputeStorageBufferRwRoConflict { .. }
        ));
    }

    #[test]
    fn validate_rejects_compute_group1_without_group0() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![
                Resource::Texture2d {
                    id: "hdr_color".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["render_attachment".into()],
                    mip_level_count: 1,
                },
                Resource::Buffer {
                    id: "g1_only".into(),
                    size: 16,
                    usage: vec!["storage".into()],
                },
            ],
            passes: vec![
                Pass::Compute {
                    ecs_before: None,
                    ecs_after: None,
                    id: "c".into(),
                    shader: "x.wgsl".into(),
                    entry_point: "m".into(),
                    dispatch: Dispatch3D { x: 1, y: 1, z: 1 },
                    texture_reads: vec![],
                    storage_writes: vec![],
                    storage_buffers: vec![],
                    storage_buffers_read: vec![],
                    storage_buffers_group1: vec!["g1_only".into()],
                    storage_buffers_read_group1: vec![],
                    indirect_dispatch: None,
                },
                Pass::RasterMesh {
                    ecs_before: None,
                    ecs_after: None,
                    id: "r".into(),
                    shader: "y.wgsl".into(),
                    vertex_entry: "vs_main".into(),
                    fragment_entry: "fs_main".into(),
                    color_targets: vec!["hdr_color".into()],
                    depth_target: None,
                },
            ],
        };
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::ComputeGroup1RequiresGroup0 { .. }
        ));
    }

    #[test]
    fn validate_rejects_duplicate_compute_storage_buffers_group1() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![
                Resource::Texture2d {
                    id: "hdr_color".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["render_attachment".into(), "texture_binding".into()],
                    mip_level_count: 1,
                },
                Resource::Texture2d {
                    id: "ping".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["storage".into()],
                    mip_level_count: 1,
                },
                Resource::Buffer {
                    id: "g1b".into(),
                    size: 16,
                    usage: vec!["storage".into()],
                },
            ],
            passes: vec![
                Pass::Compute {
                    ecs_before: None,
                    ecs_after: None,
                    id: "c".into(),
                    shader: "x.wgsl".into(),
                    entry_point: "m".into(),
                    dispatch: Dispatch3D { x: 1, y: 1, z: 1 },
                    texture_reads: vec![],
                    storage_writes: vec!["ping".into()],
                    storage_buffers: vec![],
                    storage_buffers_read: vec![],
                    storage_buffers_group1: vec!["g1b".into(), "g1b".into()],
                    storage_buffers_read_group1: vec![],
                    indirect_dispatch: None,
                },
                Pass::RasterMesh {
                    ecs_before: None,
                    ecs_after: None,
                    id: "r".into(),
                    shader: "y.wgsl".into(),
                    vertex_entry: "vs_main".into(),
                    fragment_entry: "fs_main".into(),
                    color_targets: vec!["hdr_color".into()],
                    depth_target: None,
                },
            ],
        };
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::DuplicateComputeStorageBufferGroup1 { .. }
        ));
    }

    #[test]
    fn validate_rejects_compute_buffer_in_group0_and_group1() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![
                Resource::Texture2d {
                    id: "hdr_color".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["render_attachment".into()],
                    mip_level_count: 1,
                },
                Resource::Buffer {
                    id: "shared".into(),
                    size: 64,
                    usage: vec!["storage".into()],
                },
            ],
            passes: vec![
                Pass::Compute {
                    ecs_before: None,
                    ecs_after: None,
                    id: "c".into(),
                    shader: "x.wgsl".into(),
                    entry_point: "m".into(),
                    dispatch: Dispatch3D { x: 1, y: 1, z: 1 },
                    texture_reads: vec![],
                    storage_writes: vec![],
                    storage_buffers: vec!["shared".into()],
                    storage_buffers_read: vec![],
                    storage_buffers_group1: vec!["shared".into()],
                    storage_buffers_read_group1: vec![],
                    indirect_dispatch: None,
                },
                Pass::RasterMesh {
                    ecs_before: None,
                    ecs_after: None,
                    id: "r".into(),
                    shader: "y.wgsl".into(),
                    vertex_entry: "vs_main".into(),
                    fragment_entry: "fs_main".into(),
                    color_targets: vec!["hdr_color".into()],
                    depth_target: None,
                },
            ],
        };
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::ComputeBufferSharedAcrossBindGroups { .. }
        ));
    }

    #[test]
    fn validate_accepts_compute_group1_with_group0() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![
                Resource::Texture2d {
                    id: "hdr_color".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["render_attachment".into(), "texture_binding".into()],
                    mip_level_count: 1,
                },
                Resource::Texture2d {
                    id: "ping".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["storage".into()],
                    mip_level_count: 1,
                },
                Resource::Buffer {
                    id: "g1b".into(),
                    size: 16,
                    usage: vec!["storage".into()],
                },
            ],
            passes: vec![
                Pass::Compute {
                    ecs_before: None,
                    ecs_after: None,
                    id: "c".into(),
                    shader: "x.wgsl".into(),
                    entry_point: "m".into(),
                    dispatch: Dispatch3D { x: 1, y: 1, z: 1 },
                    texture_reads: vec![],
                    storage_writes: vec!["ping".into()],
                    storage_buffers: vec![],
                    storage_buffers_read: vec![],
                    storage_buffers_group1: vec!["g1b".into()],
                    storage_buffers_read_group1: vec![],
                    indirect_dispatch: None,
                },
                Pass::RasterMesh {
                    ecs_before: None,
                    ecs_after: None,
                    id: "r".into(),
                    shader: "y.wgsl".into(),
                    vertex_entry: "vs_main".into(),
                    fragment_entry: "fs_main".into(),
                    color_targets: vec!["hdr_color".into()],
                    depth_target: None,
                },
            ],
        };
        validate_exec_v0(&doc, "hdr_color").unwrap();
    }

    #[test]
    fn validate_rejects_compute_read_group1_without_group0() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![
                Resource::Texture2d {
                    id: "hdr_color".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["render_attachment".into()],
                    mip_level_count: 1,
                },
                Resource::Buffer {
                    id: "g1_ro_only".into(),
                    size: 16,
                    usage: vec!["storage".into()],
                },
            ],
            passes: vec![
                Pass::Compute {
                    ecs_before: None,
                    ecs_after: None,
                    id: "c".into(),
                    shader: "x.wgsl".into(),
                    entry_point: "m".into(),
                    dispatch: Dispatch3D { x: 1, y: 1, z: 1 },
                    texture_reads: vec![],
                    storage_writes: vec![],
                    storage_buffers: vec![],
                    storage_buffers_read: vec![],
                    storage_buffers_group1: vec![],
                    storage_buffers_read_group1: vec!["g1_ro_only".into()],
                    indirect_dispatch: None,
                },
                Pass::RasterMesh {
                    ecs_before: None,
                    ecs_after: None,
                    id: "r".into(),
                    shader: "y.wgsl".into(),
                    vertex_entry: "vs_main".into(),
                    fragment_entry: "fs_main".into(),
                    color_targets: vec!["hdr_color".into()],
                    depth_target: None,
                },
            ],
        };
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::ComputeGroup1RequiresGroup0 { .. }
        ));
    }

    #[test]
    fn validate_rejects_duplicate_compute_storage_buffers_read_group1() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![
                Resource::Texture2d {
                    id: "hdr_color".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["render_attachment".into(), "texture_binding".into()],
                    mip_level_count: 1,
                },
                Resource::Texture2d {
                    id: "ping".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["storage".into()],
                    mip_level_count: 1,
                },
                Resource::Buffer {
                    id: "g1r".into(),
                    size: 16,
                    usage: vec!["storage".into()],
                },
            ],
            passes: vec![
                Pass::Compute {
                    ecs_before: None,
                    ecs_after: None,
                    id: "c".into(),
                    shader: "x.wgsl".into(),
                    entry_point: "m".into(),
                    dispatch: Dispatch3D { x: 1, y: 1, z: 1 },
                    texture_reads: vec![],
                    storage_writes: vec!["ping".into()],
                    storage_buffers: vec![],
                    storage_buffers_read: vec![],
                    storage_buffers_group1: vec![],
                    storage_buffers_read_group1: vec!["g1r".into(), "g1r".into()],
                    indirect_dispatch: None,
                },
                Pass::RasterMesh {
                    ecs_before: None,
                    ecs_after: None,
                    id: "r".into(),
                    shader: "y.wgsl".into(),
                    vertex_entry: "vs_main".into(),
                    fragment_entry: "fs_main".into(),
                    color_targets: vec!["hdr_color".into()],
                    depth_target: None,
                },
            ],
        };
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::DuplicateComputeStorageBufferReadGroup1 { .. }
        ));
    }

    #[test]
    fn validate_rejects_compute_group1_same_buffer_rw_and_ro() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![
                Resource::Texture2d {
                    id: "hdr_color".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["render_attachment".into(), "texture_binding".into()],
                    mip_level_count: 1,
                },
                Resource::Texture2d {
                    id: "ping".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["storage".into()],
                    mip_level_count: 1,
                },
                Resource::Buffer {
                    id: "g1x".into(),
                    size: 16,
                    usage: vec!["storage".into()],
                },
            ],
            passes: vec![
                Pass::Compute {
                    ecs_before: None,
                    ecs_after: None,
                    id: "c".into(),
                    shader: "x.wgsl".into(),
                    entry_point: "m".into(),
                    dispatch: Dispatch3D { x: 1, y: 1, z: 1 },
                    texture_reads: vec![],
                    storage_writes: vec!["ping".into()],
                    storage_buffers: vec![],
                    storage_buffers_read: vec![],
                    storage_buffers_group1: vec!["g1x".into()],
                    storage_buffers_read_group1: vec!["g1x".into()],
                    indirect_dispatch: None,
                },
                Pass::RasterMesh {
                    ecs_before: None,
                    ecs_after: None,
                    id: "r".into(),
                    shader: "y.wgsl".into(),
                    vertex_entry: "vs_main".into(),
                    fragment_entry: "fs_main".into(),
                    color_targets: vec!["hdr_color".into()],
                    depth_target: None,
                },
            ],
        };
        let e = validate_exec_v0(&doc, "hdr_color").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::ComputeStorageBufferGroup1RwRoConflict { .. }
        ));
    }

    #[test]
    fn validate_accepts_compute_group1_ro_only_with_group0() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![
                Resource::Texture2d {
                    id: "hdr_color".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["render_attachment".into(), "texture_binding".into()],
                    mip_level_count: 1,
                },
                Resource::Texture2d {
                    id: "ping".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["storage".into()],
                    mip_level_count: 1,
                },
                Resource::Buffer {
                    id: "g1r".into(),
                    size: 16,
                    usage: vec!["storage".into()],
                },
            ],
            passes: vec![
                Pass::Compute {
                    ecs_before: None,
                    ecs_after: None,
                    id: "c".into(),
                    shader: "x.wgsl".into(),
                    entry_point: "m".into(),
                    dispatch: Dispatch3D { x: 1, y: 1, z: 1 },
                    texture_reads: vec![],
                    storage_writes: vec!["ping".into()],
                    storage_buffers: vec![],
                    storage_buffers_read: vec![],
                    storage_buffers_group1: vec![],
                    storage_buffers_read_group1: vec!["g1r".into()],
                    indirect_dispatch: None,
                },
                Pass::RasterMesh {
                    ecs_before: None,
                    ecs_after: None,
                    id: "r".into(),
                    shader: "y.wgsl".into(),
                    vertex_entry: "vs_main".into(),
                    fragment_entry: "fs_main".into(),
                    color_targets: vec!["hdr_color".into()],
                    depth_target: None,
                },
            ],
        };
        validate_exec_v0(&doc, "hdr_color").unwrap();
    }

    #[test]
    fn validate_rejects_blit_same_texture() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![Resource::Texture2d {
                id: "t".into(),
                format: "Rgba16Float".into(),
                width: 4,
                height: 4,
                usage: vec![
                    "render_attachment".into(),
                    "copy_src".into(),
                    "copy_dst".into(),
                ],
                mip_level_count: 1,
            }],
            passes: vec![
                Pass::Blit {
                    ecs_before: None,
                    ecs_after: None,
                    id: "b".into(),
                    source: "t".into(),
                    destination: "t".into(),
                    region: None,
                },
                Pass::RasterMesh {
                    ecs_before: None,
                    ecs_after: None,
                    id: "r".into(),
                    shader: "y.wgsl".into(),
                    vertex_entry: "vs_main".into(),
                    fragment_entry: "fs_main".into(),
                    color_targets: vec!["t".into()],
                    depth_target: None,
                },
            ],
        };
        let e = validate_exec_v0(&doc, "t").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::BlitSameTexture { .. }
        ));
    }

    #[test]
    fn validate_rejects_blit_format_mismatch() {
        let doc = RenderGraphDocument {
            schema: "w3drs.render_graph".into(),
            version: 1,
            resources: vec![
                Resource::Texture2d {
                    id: "a".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["copy_src".into(), "render_attachment".into()],
                    mip_level_count: 1,
                },
                Resource::Texture2d {
                    id: "b".into(),
                    format: "Rgba8Unorm".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["copy_dst".into(), "render_attachment".into()],
                    mip_level_count: 1,
                },
            ],
            passes: vec![
                Pass::Blit {
                    ecs_before: None,
                    ecs_after: None,
                    id: "blit".into(),
                    source: "a".into(),
                    destination: "b".into(),
                    region: None,
                },
                Pass::RasterMesh {
                    ecs_before: None,
                    ecs_after: None,
                    id: "r".into(),
                    shader: "y.wgsl".into(),
                    vertex_entry: "vs_main".into(),
                    fragment_entry: "fs_main".into(),
                    color_targets: vec!["a".into()],
                    depth_target: None,
                },
            ],
        };
        let e = validate_exec_v0(&doc, "a").unwrap_err();
        assert!(matches!(
            e,
            RenderGraphValidateError::BlitFormatMismatch { .. }
        ));
    }
}
