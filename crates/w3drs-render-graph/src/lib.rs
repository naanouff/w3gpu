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
        /// Nombre de mips alloués (≥ 1). Requis pour `blit.region` avec `src_mip_level` / `dst_mip_level` > 0.
        #[serde(default = "default_mip_level_count")]
        mip_level_count: u32,
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
        /// Buffers bound as **read/write storage** in group 0 (`binding` = index in this list) — `storage` usage requis sur la ressource.
        #[serde(default)]
        storage_buffers: Vec<String>,
        /// Buffers bound as **read-only storage** in group 0, **after** `storage_buffers` (bindings continues).
        #[serde(default)]
        storage_buffers_read: Vec<String>,
        /// Read/write **storage** buffers in **bind group 1** (`@group(1)`), `binding` = index in this list. Puis **`storage_buffers_read_group1`** (ro) avec des `binding` qui **continuent** après cette liste.
        #[serde(default)]
        storage_buffers_group1: Vec<String>,
        /// Read-only **storage** buffers in **bind group 1**, après `storage_buffers_group1` (mêmes règles d’usage `storage` ; pas de doublon ; interdit si le buffer est déjà en groupe 0 ou en rw groupe 1).
        #[serde(default)]
        storage_buffers_read_group1: Vec<String>,
        /// Si présent, `dispatch_workgroups_indirect` sur ce buffer (`offset` … `+12` octets, 3×`u32` aligné 4) ; sinon `dispatch` fixe.
        #[serde(default)]
        indirect_dispatch: Option<IndirectDispatchArgs>,
        /// B.6 : optional label; host encodes `ecs_node` before this pass (native / embedded runner).
        #[serde(default)]
        ecs_before: Option<String>,
        #[serde(default)]
        ecs_after: Option<String>,
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
        #[serde(default)]
        ecs_before: Option<String>,
        #[serde(default)]
        ecs_after: Option<String>,
    },
    /// Fullscreen triangle raster pass (same attachment model as `raster_mesh` in v0).
    #[serde(rename = "fullscreen")]
    Fullscreen {
        id: String,
        shader: String,
        vertex_entry: String,
        fragment_entry: String,
        #[serde(default)]
        color_targets: Vec<String>,
        #[serde(default)]
        depth_target: Option<String>,
        #[serde(default)]
        ecs_before: Option<String>,
        #[serde(default)]
        ecs_after: Option<String>,
    },
    /// **B.7** : depth-only mesh pass (no color targets) — `shadow_depth`-style: group(0) uniform, group(1) read-only instance matrices. Draws are **host**-encoded (see `RenderGraphV0Host` in `w3drs-renderer`).
    #[serde(rename = "raster_depth_mesh")]
    RasterDepthMesh {
        id: String,
        shader: String,
        vertex_entry: String,
        /// Depth target only (e.g. `Depth32Float`).
        depth_target: String,
        /// `group(0) @binding(0)` — uniform (e.g. 80B `LightUniforms`).
        light_uniforms_buffer: String,
        /// `group(1) @binding(0)` — `storage, read` instance `mat4x4` array.
        instance_buffer: String,
        #[serde(default)]
        ecs_before: Option<String>,
        #[serde(default)]
        ecs_after: Option<String>,
    },
    /// Copie texture → texture (`copy_texture_to_texture`). Sans `region` : mip 0 entier, mêmes format et taille logique qu’aujourd’hui. Avec `region` : sous-rectangle et mips (voir `BlitRegion`).
    #[serde(rename = "blit")]
    Blit {
        id: String,
        source: String,
        destination: String,
        #[serde(default)]
        region: Option<BlitRegion>,
        #[serde(default)]
        ecs_before: Option<String>,
        #[serde(default)]
        ecs_after: Option<String>,
    },
}

impl Pass {
    /// Submission-order pass id.
    pub fn id(&self) -> &str {
        match self {
            Pass::Compute { id, .. }
            | Pass::RasterMesh { id, .. }
            | Pass::Fullscreen { id, .. }
            | Pass::Blit { id, .. }
            | Pass::RasterDepthMesh { id, .. } => id.as_str(),
        }
    }

    /// B.6 : run host `ecs_node` with this label **before** encoding the pass (if `Some` and non-empty).
    pub fn ecs_before_label(&self) -> Option<&str> {
        let s = match self {
            Pass::Compute { ecs_before, .. }
            | Pass::RasterMesh { ecs_before, .. }
            | Pass::Fullscreen { ecs_before, .. }
            | Pass::Blit { ecs_before, .. }
            | Pass::RasterDepthMesh { ecs_before, .. } => ecs_before.as_deref(),
        };
        s.filter(|l| !l.is_empty())
    }

    /// B.6 : run host `ecs_node` with this label **after** encoding the pass (if `Some` and non-empty).
    pub fn ecs_after_label(&self) -> Option<&str> {
        let s = match self {
            Pass::Compute { ecs_after, .. }
            | Pass::RasterMesh { ecs_after, .. }
            | Pass::Fullscreen { ecs_after, .. }
            | Pass::Blit { ecs_after, .. }
            | Pass::RasterDepthMesh { ecs_after, .. } => ecs_after.as_deref(),
        };
        s.filter(|l| !l.is_empty())
    }
}

fn default_mip_level_count() -> u32 {
    1
}

/// Arguments pour `dispatch_workgroups_indirect` (12 octets : `x`, `y`, `z` en `u32` little-endian).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct IndirectDispatchArgs {
    pub buffer: String,
    #[serde(default)]
    pub offset: u64,
}

/// Sous-copie optionnelle pour une passe `blit` (origines en texels, taille implicite = reste valide si `width` / `height` absents).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BlitRegion {
    #[serde(default)]
    pub src_mip_level: u32,
    #[serde(default)]
    pub dst_mip_level: u32,
    #[serde(default)]
    pub src_origin_x: u32,
    #[serde(default)]
    pub src_origin_y: u32,
    #[serde(default)]
    pub dst_origin_x: u32,
    #[serde(default)]
    pub dst_origin_y: u32,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
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
                storage_buffers: _,
                storage_buffers_read: _,
                ..
            } => {
                if dispatch.x == 0 || dispatch.y == 0 || dispatch.z == 0 {
                    return Err(RenderGraphError::ZeroDispatch {
                        pass_id: id.clone(),
                    });
                }
                id.as_str()
            }
            Pass::RasterMesh { id, .. }
            | Pass::Fullscreen { id, .. }
            | Pass::Blit { id, .. }
            | Pass::RasterDepthMesh { id, .. } => id.as_str(),
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
        assert_eq!(doc.resources.len(), 8);
        assert_eq!(doc.passes.len(), 4);
        assert!(matches!(doc.passes[0], Pass::Compute { .. }));
        assert!(matches!(doc.passes[1], Pass::RasterMesh { .. }));
        assert!(matches!(doc.passes[2], Pass::Fullscreen { .. }));
        assert!(matches!(doc.passes[3], Pass::Blit { .. }));
    }

    #[test]
    fn rejects_wrong_schema() {
        let err = parse_render_graph_json(r#"{"schema":"other","version":1,"resources":[],"passes":[{"kind":"compute","id":"a","shader":"s.wgsl","entry_point":"m","dispatch":{"x":1,"y":1,"z":1}}]}"#)
            .unwrap_err();
        assert!(matches!(err, RenderGraphError::UnsupportedSchema(_)));
    }
}
