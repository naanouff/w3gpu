//! Phase B — run [`fixtures/phases/phase-b/render_graph.json`](../../fixtures/phases/phase-b/render_graph.json)
//! twice and assert identical pixel checksum (DOD-oriented smoke).

use std::path::PathBuf;

use w3drs_render_graph::{Pass, Resource};
use w3drs_renderer::{
    parse_render_graph_json, run_graph_v0_checksum, run_graph_v0_checksum_with_registry,
    validate_render_graph_exec_v0, RenderGraphExecError, RenderGraphGpuRegistry,
};

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

fn try_gpu() -> Option<Gpu> {
    pollster::block_on(async {
        let inst = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = inst
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::None,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .ok()?;
        Some(Gpu { device, queue })
    })
}

fn phase_b_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/phases/phase-b")
}

#[test]
fn phase_b_graph_checksum_stable_over_two_frames() {
    let Some(gpu) = try_gpu() else {
        return;
    };
    let dir = phase_b_dir();
    let json = std::fs::read_to_string(dir.join("render_graph.json")).expect("fixture");
    let doc = parse_render_graph_json(&json).expect("parse");

    let a = run_graph_v0_checksum(&gpu.device, &gpu.queue, &doc, &dir, "hdr_color")
        .expect("run 1");
    let b = run_graph_v0_checksum(&gpu.device, &gpu.queue, &doc, &dir, "hdr_color")
        .expect("run 2");
    assert_eq!(a, b, "two identical submits must yield the same readback hash");
}

#[test]
fn phase_b_graph_registry_resize_unknown_errors() {
    let Some(gpu) = try_gpu() else {
        return;
    };
    let dir = phase_b_dir();
    let json = std::fs::read_to_string(dir.join("render_graph.json")).expect("fixture");
    let doc = parse_render_graph_json(&json).expect("parse");
    let mut reg = RenderGraphGpuRegistry::new(&gpu.device, &doc).expect("registry");
    let err = reg
        .resize_texture_2d(&gpu.device, "no_such_tex", 4, 4)
        .unwrap_err();
    assert!(
        matches!(err, RenderGraphExecError::MissingResource(ref s) if s.contains("no_such_tex")),
        "got {err:?}"
    );
}

#[test]
fn phase_b_graph_resize_smaller_texture_checksum_stable_twice() {
    let Some(gpu) = try_gpu() else {
        return;
    };
    let dir = phase_b_dir();
    let json = std::fs::read_to_string(dir.join("render_graph.json")).expect("fixture");
    let mut doc = parse_render_graph_json(&json).expect("parse");
    for r in &mut doc.resources {
        if let Resource::Texture2d {
            id,
            ref mut width,
            ref mut height,
            ..
        } = r
        {
            if id == "hdr_color" || id == "scene_depth" {
                *width = 32;
                *height = 32;
            }
        }
    }
    validate_render_graph_exec_v0(&doc, "hdr_color").expect("validate");
    let mut reg = RenderGraphGpuRegistry::new(&gpu.device, &doc).expect("registry");
    reg
        .resize_texture_2d(&gpu.device, "hdr_color", 16, 16)
        .expect("resize hdr_color");
    reg
        .resize_texture_2d(&gpu.device, "scene_depth", 16, 16)
        .expect("resize scene_depth");
    let a = run_graph_v0_checksum_with_registry(
        &gpu.device,
        &gpu.queue,
        &doc,
        &dir,
        "hdr_color",
        &reg,
    )
    .expect("run 1");
    let b = run_graph_v0_checksum_with_registry(
        &gpu.device,
        &gpu.queue,
        &doc,
        &dir,
        "hdr_color",
        &reg,
    )
    .expect("run 2");
    assert_eq!(a, b, "checksum must match after resize + two runs");
}

#[test]
fn phase_b_graph_registry_buffer_lookup() {
    let Some(gpu) = try_gpu() else {
        return;
    };
    let dir = phase_b_dir();
    let json = std::fs::read_to_string(dir.join("render_graph.json")).expect("fixture");
    let doc = parse_render_graph_json(&json).expect("parse");
    let reg = RenderGraphGpuRegistry::new(&gpu.device, &doc).expect("registry");
    assert!(reg.buffer("indirect_args").is_ok());
}

#[test]
fn phase_b_graph_missing_shader_returns_io() {
    let Some(gpu) = try_gpu() else {
        return;
    };
    let dir = phase_b_dir();
    let json = std::fs::read_to_string(dir.join("render_graph.json")).expect("fixture");
    let mut doc = parse_render_graph_json(&json).expect("parse");
    if let Pass::Compute { ref mut shader, .. } = doc.passes[0] {
        *shader = "shaders/does_not_exist.wgsl".into();
    } else {
        panic!("expected compute first");
    }
    validate_render_graph_exec_v0(&doc, "hdr_color").expect("validate still ok");
    let err = run_graph_v0_checksum(&gpu.device, &gpu.queue, &doc, &dir, "hdr_color").unwrap_err();
    assert!(
        matches!(err, RenderGraphExecError::Io(_)),
        "expected Io error, got {err:?}"
    );
}
