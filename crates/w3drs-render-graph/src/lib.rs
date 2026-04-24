//! Declarative **render graph** documents (Phase B).
//!
//! v0 scope: **parse + validate** JSON (`schema` / `version`, resources, ordered passes).
//! Runtime execution on `wgpu` lives in `w3drs-renderer` (future).
//! **Execution-time validation** (no GPU): [`validate_exec_v0`](exec_validate::validate_exec_v0).

mod exec_validate;

use serde::Deserialize;

pub use exec_validate::{pass_ids_in_order_v0, validate_exec_v0, RenderGraphValidateError};

const SCHEMA_ID: &str = "w3drs.render_graph";
const SUPPORTED_VERSION: u32 = 1;

/// Top-level document on disk (e.g. `fixtures/phases/phase-b/render_graph.json`).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct RenderGraphDocument {
    pub schema: String,
    pub version: u32,
    pub resources: Vec<Resource>,
    pub passes: Vec<Pass>,
}

/// GPU resource declared by the graph (v0 — descriptive only).
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum Resource {
    #[serde(rename = "texture_2d")]
    Texture2d {
        id: String,
        format: String,
        width: u32,
        height: u32,
        #[serde(default)]
        usage: Vec<String>,
    },
    #[serde(rename = "buffer")]
    Buffer {
        id: String,
        size: u64,
        #[serde(default)]
        usage: Vec<String>,
    },
}

/// A single pass in **submission order**.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum Pass {
    #[serde(rename = "compute")]
    Compute {
        id: String,
        /// Path relative to the graph file (workspace convention).
        shader: String,
        entry_point: String,
        dispatch: Dispatch3D,
        /// Textures **sampled** from this compute pass (WGSL `texture` / `texture_binding`).
        #[serde(default)]
        texture_reads: Vec<String>,
        /// Textures written as **storage** images in this pass (`storage` usage on the resource).
        #[serde(default)]
        storage_writes: Vec<String>,
    },
    #[serde(rename = "raster_mesh")]
    RasterMesh {
        id: String,
        shader: String,
        vertex_entry: String,
        fragment_entry: String,
        #[serde(default)]
        color_targets: Vec<String>,
        #[serde(default)]
        depth_target: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub struct Dispatch3D {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum RenderGraphError {
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported schema {0:?} (expected {SCHEMA_ID:?})")]
    UnsupportedSchema(String),
    #[error("unsupported version {0} (expected {SUPPORTED_VERSION})")]
    UnsupportedVersion(u32),
    #[error("empty pass list")]
    EmptyPasses,
    #[error("duplicate resource id {0:?}")]
    DuplicateResourceId(String),
    #[error("duplicate pass id {0:?}")]
    DuplicatePassId(String),
    #[error("dispatch dimension must be > 0 (pass {pass_id})")]
    ZeroDispatch { pass_id: String },
}

/// Parse JSON and apply v0 validation rules.
pub fn parse_render_graph_json(json: &str) -> Result<RenderGraphDocument, RenderGraphError> {
    let doc: RenderGraphDocument = serde_json::from_str(json)?;
    validate(&doc)?;
    Ok(doc)
}

fn validate(doc: &RenderGraphDocument) -> Result<(), RenderGraphError> {
    if doc.schema != SCHEMA_ID {
        return Err(RenderGraphError::UnsupportedSchema(doc.schema.clone()));
    }
    if doc.version != SUPPORTED_VERSION {
        return Err(RenderGraphError::UnsupportedVersion(doc.version));
    }
    if doc.passes.is_empty() {
        return Err(RenderGraphError::EmptyPasses);
    }
    let mut seen = std::collections::HashSet::<&str>::new();
    for r in &doc.resources {
        let id = match r {
            Resource::Texture2d { id, .. } | Resource::Buffer { id, .. } => id.as_str(),
        };
        if !seen.insert(id) {
            return Err(RenderGraphError::DuplicateResourceId(id.to_string()));
        }
    }
    let mut seen_pass = std::collections::HashSet::<&str>::new();
    for p in &doc.passes {
        let id = match p {
            Pass::Compute {
                id,
                dispatch,
                texture_reads: _,
                storage_writes: _,
                ..
            } => {
                if dispatch.x == 0 || dispatch.y == 0 || dispatch.z == 0 {
                    return Err(RenderGraphError::ZeroDispatch {
                        pass_id: id.clone(),
                    });
                }
                id.as_str()
            }
            Pass::RasterMesh { id, .. } => id.as_str(),
        };
        if !seen_pass.insert(id) {
            return Err(RenderGraphError::DuplicatePassId(id.to_string()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn phase_b_fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/phases/phase-b/render_graph.json")
    }

    #[test]
    fn parse_phase_b_fixture() {
        let json = std::fs::read_to_string(phase_b_fixture()).expect("read fixture");
        let doc = parse_render_graph_json(&json).expect("valid graph");
        assert_eq!(doc.schema, SCHEMA_ID);
        assert_eq!(doc.version, 1);
        assert_eq!(doc.resources.len(), 3);
        assert_eq!(doc.passes.len(), 2);
        assert!(matches!(doc.passes[0], Pass::Compute { .. }));
        assert!(matches!(doc.passes[1], Pass::RasterMesh { .. }));
    }

    #[test]
    fn rejects_wrong_schema() {
        let err = parse_render_graph_json(r#"{"schema":"other","version":1,"resources":[],"passes":[{"kind":"compute","id":"a","shader":"s.wgsl","entry_point":"m","dispatch":{"x":1,"y":1,"z":1}}]}"#)
            .unwrap_err();
        assert!(matches!(err, RenderGraphError::UnsupportedSchema(_)));
    }
}
