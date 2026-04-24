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
}

fn texture_format_known(s: &str) -> Result<(), RenderGraphValidateError> {
    match s {
        "Rgba16Float" | "Rgba8Unorm" => Ok(()),
        "Depth24Plus"
        | "Depth32Float"
        | "Depth24PlusStencil8"
        | "Depth32FloatStencil8" => Ok(()),
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

fn texture_resource<'a>(
    doc: &'a RenderGraphDocument,
    id: &str,
) -> Option<&'a Resource> {
    doc.resources.iter().find(|r| match r {
        Resource::Texture2d { id: tid, .. } => tid == id,
        Resource::Buffer { .. } => false,
    })
}

/// Ordered pass ids (submission order) — input to a future explicit barrier planner.
pub fn pass_ids_in_order_v0(doc: &RenderGraphDocument) -> Vec<&str> {
    doc.passes
        .iter()
        .map(|p| match p {
            Pass::Compute { id, .. } | Pass::RasterMesh { id, .. } => id.as_str(),
        })
        .collect()
}

fn validate_pass_resource_semantics_v0(doc: &RenderGraphDocument) -> Result<(), RenderGraphValidateError> {
    let mut texture_ids = std::collections::HashSet::<String>::new();
    for r in &doc.resources {
        if let Resource::Texture2d { id, .. } = r {
            texture_ids.insert(id.clone());
        }
    }

    for p in &doc.passes {
        match p {
            Pass::Compute {
                id: pass_id,
                texture_reads,
                storage_writes,
                ..
            } => {
                for tid in texture_reads {
                    if !texture_ids.contains(tid) {
                        return Err(RenderGraphValidateError::MissingResource(tid.clone()));
                    }
                    let Some(Resource::Texture2d { usage, .. }) = texture_resource(doc, tid) else {
                        continue;
                    };
                    if !usage_has(usage, "texture_binding") {
                        return Err(RenderGraphValidateError::TextureSemanticUsageMissing {
                            pass_id: pass_id.clone(),
                            texture_id: tid.clone(),
                            missing: "texture_binding".into(),
                            declared: usage.clone(),
                        });
                    }
                }
                for tid in storage_writes {
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
            }
            Pass::RasterMesh {
                id: pass_id,
                color_targets,
                depth_target,
                ..
            } => {
                let mut seen_ct = std::collections::HashSet::<&str>::new();
                for ct in color_targets {
                    if !seen_ct.insert(ct.as_str()) {
                        return Err(RenderGraphValidateError::DuplicateRasterColorTarget {
                            pass_id: pass_id.clone(),
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
                            pass_id: pass_id.clone(),
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
                    if let Some(Resource::Texture2d { format, usage, .. }) = texture_resource(doc, dt)
                    {
                        if !is_depth_stencil_format(format.as_str()) {
                            return Err(RenderGraphValidateError::InvalidDepthTargetFormat {
                                texture_id: dt.clone(),
                                got: format.clone(),
                            });
                        }
                        if !usage_has(usage, "render_attachment") {
                            return Err(RenderGraphValidateError::TextureSemanticUsageMissing {
                                pass_id: pass_id.clone(),
                                texture_id: dt.clone(),
                                missing: "render_attachment".into(),
                                declared: usage.clone(),
                            });
                        }
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
            "storage" | "copy_dst" | "copy_src" | "map_read" => {}
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
                ..
            } => {
                texture_format_known(format)?;
                texture_usage_known(usage)?;
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
    if let Some(Resource::Texture2d { format, .. }) = doc.resources.iter().find(|r| {
        matches!(r, Resource::Texture2d { id, .. } if id == readback_id)
    }) {
        if format != "Rgba16Float" {
            return Err(RenderGraphValidateError::InvalidReadbackFormat {
                id: readback_id.to_string(),
                got: format.clone(),
            });
        }
    }

    for p in &doc.passes {
        if let Pass::RasterMesh {
            id,
            color_targets,
            ..
        } = p
        {
            if color_targets.is_empty() {
                return Err(RenderGraphValidateError::EmptyColorTargets(id.clone()));
            }
            for ct in color_targets {
                if !texture_ids.contains(ct) {
                    return Err(RenderGraphValidateError::MissingResource(ct.clone()));
                }
            }
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
                },
                Resource::Buffer {
                    id: "b".into(),
                    size: 16,
                    usage: buf_usage,
                },
            ],
            passes: vec![
                Pass::Compute {
                    id: "c".into(),
                    shader: "shaders/x.wgsl".into(),
                    entry_point: "cs_main".into(),
                    dispatch: Dispatch3D { x: 1, y: 1, z: 1 },
                    texture_reads: vec![],
                    storage_writes: vec![],
                },
                Pass::RasterMesh {
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
            }],
            passes: vec![Pass::RasterMesh {
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
            }],
            passes: vec![Pass::RasterMesh {
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
            vec![
                "texture_binding".into(),
                "render_attachment".into(),
            ],
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
            }],
            passes: vec![Pass::RasterMesh {
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
            }],
            passes: vec![Pass::RasterMesh {
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
                },
                Resource::Texture2d {
                    id: "work".into(),
                    format: "Rgba16Float".into(),
                    width: 4,
                    height: 4,
                    usage: vec!["texture_binding".into()],
                },
            ],
            passes: vec![
                Pass::Compute {
                    id: "c".into(),
                    shader: "x.wgsl".into(),
                    entry_point: "m".into(),
                    dispatch: Dispatch3D { x: 1, y: 1, z: 1 },
                    texture_reads: vec![],
                    storage_writes: vec!["work".into()],
                },
                Pass::RasterMesh {
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
}
