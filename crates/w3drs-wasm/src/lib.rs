#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;

use w3drs_render_graph::{parse_render_graph_json, validate_exec_v0};

mod engine;

pub use engine::{HdrLoadStats, W3drsEngine};

/// Parse JSON graphe v1 + validation exécution v0 (même règles que le natif avant checksum).
/// JS : `w3drsValidateRenderGraphV0(json, readbackId)`.
#[wasm_bindgen(js_name = w3drsValidateRenderGraphV0)]
pub fn w3drs_validate_render_graph_v0(json: &str, readback_id: &str) -> Result<(), JsValue> {
    let doc = parse_render_graph_json(json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    validate_exec_v0(&doc, readback_id).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen(start)]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Debug);
}
