//! Phase B — run [`fixtures/phases/phase-b/render_graph.json`](../../fixtures/phases/phase-b/render_graph.json)
//! twice and assert identical pixel checksum (DOD-oriented smoke).

use std::path::PathBuf;

use w3drs_render_graph::{BlitRegion, IndirectDispatchArgs, Pass, Resource};
use w3drs_renderer::{
    parse_render_graph_json, run_graph_v0_checksum, run_graph_v0_checksum_with_registry,
    run_graph_v0_checksum_with_registry_pre_writes, run_graph_v0_checksum_with_registry_wgsl_host,
    validate_render_graph_exec_v0, RenderGraphExecError, RenderGraphGpuRegistry, RenderGraphV0Host,
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
            .await
            .ok()?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
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

    let a = run_graph_v0_checksum(&gpu.device, &gpu.queue, &doc, &dir, "hdr_color").expect("run 1");
    let b = run_graph_v0_checksum(&gpu.device, &gpu.queue, &doc, &dir, "hdr_color").expect("run 2");
    assert_eq!(
        a, b,
        "two identical submits must yield the same readback hash"
    );
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
            if id == "hdr_color" || id == "hdr_blit_dst" || id == "scene_depth" {
                *width = 32;
                *height = 32;
            }
        }
    }
    validate_render_graph_exec_v0(&doc, "hdr_color").expect("validate");
    let mut reg = RenderGraphGpuRegistry::new(&gpu.device, &doc).expect("registry");
    reg.resize_texture_2d(&gpu.device, "hdr_color", 16, 16)
        .expect("resize hdr_color");
    reg.resize_texture_2d(&gpu.device, "scene_depth", 16, 16)
        .expect("resize scene_depth");
    reg.resize_texture_2d(&gpu.device, "hdr_blit_dst", 16, 16)
        .expect("resize hdr_blit_dst");
    let a =
        run_graph_v0_checksum_with_registry(&gpu.device, &gpu.queue, &doc, &dir, "hdr_color", &reg)
            .expect("run 1");
    let b =
        run_graph_v0_checksum_with_registry(&gpu.device, &gpu.queue, &doc, &dir, "hdr_color", &reg)
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
fn phase_b_graph_checksum_matches_with_compute_indirect_dispatch_seeded() {
    let Some(gpu) = try_gpu() else {
        return;
    };
    let dir = phase_b_dir();
    let json = std::fs::read_to_string(dir.join("render_graph.json")).expect("fixture");
    let mut doc = parse_render_graph_json(&json).expect("parse");
    let baseline =
        run_graph_v0_checksum(&gpu.device, &gpu.queue, &doc, &dir, "hdr_color").expect("baseline");

    for r in &mut doc.resources {
        if let Resource::Buffer { id, usage, .. } = r {
            if id == "indirect_args" && !usage.iter().any(|u| u == "indirect") {
                usage.push("indirect".into());
            }
        }
    }
    if let Pass::Compute {
        indirect_dispatch, ..
    } = &mut doc.passes[0]
    {
        *indirect_dispatch = Some(IndirectDispatchArgs {
            buffer: "indirect_args".into(),
            offset: 0,
        });
    } else {
        panic!("expected compute first");
    }
    validate_render_graph_exec_v0(&doc, "hdr_color").expect("validate indirect variant");
    let registry = RenderGraphGpuRegistry::new(&gpu.device, &doc).expect("registry");
    let indirect_xyz: [u8; 12] = [4, 0, 0, 0, 4, 0, 0, 0, 1, 0, 0, 0];
    let with_indirect = run_graph_v0_checksum_with_registry_pre_writes(
        &gpu.device,
        &gpu.queue,
        &doc,
        &dir,
        "hdr_color",
        &registry,
        &[("indirect_args", 0, &indirect_xyz)],
    )
    .expect("run indirect");
    assert_eq!(
        baseline, with_indirect,
        "indirect dispatch (4,4,1) seeded in buffer must match fixed dispatch"
    );
}

#[test]
fn phase_b_graph_checksum_matches_when_blit_declares_full_mip0_region() {
    let Some(gpu) = try_gpu() else {
        return;
    };
    let dir = phase_b_dir();
    let json = std::fs::read_to_string(dir.join("render_graph.json")).expect("fixture");
    let mut doc = parse_render_graph_json(&json).expect("parse");
    let baseline =
        run_graph_v0_checksum(&gpu.device, &gpu.queue, &doc, &dir, "hdr_color").expect("baseline");

    let n = doc.passes.len();
    if let Pass::Blit { region, .. } = &mut doc.passes[n - 1] {
        *region = Some(BlitRegion {
            src_mip_level: 0,
            dst_mip_level: 0,
            src_origin_x: 0,
            src_origin_y: 0,
            dst_origin_x: 0,
            dst_origin_y: 0,
            width: None,
            height: None,
        });
    } else {
        panic!("expected blit last");
    }
    validate_render_graph_exec_v0(&doc, "hdr_color").expect("validate blit region");
    let with_region =
        run_graph_v0_checksum(&gpu.device, &gpu.queue, &doc, &dir, "hdr_color").expect("run");
    assert_eq!(
        baseline, with_region,
        "explicit full mip0 blit region must match default full texture copy"
    );
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

/// B.6 nœuds ECS (labels) + B.7 encode `raster_depth_mesh` (callback hôte) — v0.
#[derive(Default)]
struct B67TestHost {
    ecs_labels: Vec<String>,
    depth_pass_draw_calls: u32,
}

impl RenderGraphV0Host for B67TestHost {
    fn ecs_node(&mut self, label: &str) {
        self.ecs_labels.push(label.to_string());
    }

    fn draw_raster_depth_mesh(&mut self, _pass_id: &str, _rpass: &mut wgpu::RenderPass<'_>) {
        self.depth_pass_draw_calls += 1;
    }
}

#[test]
fn phase_b_b6_b7_raster_depth_mesh_host_hooks_and_depth_encode() {
    let Some(gpu) = try_gpu() else {
        return;
    };
    let dir = phase_b_dir();
    let json = std::fs::read_to_string(dir.join("b67_raster_depth_test.json")).expect("b67");
    let doc = parse_render_graph_json(&json).expect("parse");
    validate_render_graph_exec_v0(&doc, "hdr_readback").expect("validate");
    let registry = RenderGraphGpuRegistry::new(&gpu.device, &doc).expect("registry");
    let mut load =
        |rel: &str| std::fs::read_to_string(dir.join(rel)).map_err(RenderGraphExecError::from);
    let mut host = B67TestHost::default();
    futures_executor::block_on(run_graph_v0_checksum_with_registry_wgsl_host(
        &gpu.device,
        &gpu.queue,
        &doc,
        "hdr_readback",
        &registry,
        &[],
        &mut load,
        &mut host,
    ))
    .expect("graph with raster_depth_mesh + readback");
    assert_eq!(host.ecs_labels, &["b6_test_before", "b6_test_after"]);
    assert_eq!(host.depth_pass_draw_calls, 1);
}
