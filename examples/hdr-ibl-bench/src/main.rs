//! Même `load_hdr_from_bytes` + `IblContext::from_hdr_with_spec` que le viewer (natif / wasm).
//! Pas de bind group d’environnement — cœur IBL seulement.
//!
//! Args : `[--tier=max|high|medium|low|min] [chemin.hdr]` (fichier par défaut : `www/public/studio_small_03_2k.hdr`).

use std::path::Path;
use w3drs_assets::load_hdr_from_bytes;
use w3drs_renderer::{IblContext, IblGenerationSpec};

fn main() {
    let default_hdr = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("www")
        .join("public")
        .join("studio_small_03_2k.hdr");

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut tier: String = "max".to_string();
    let mut path_opt: Option<std::path::PathBuf> = None;
    let mut i = 0usize;
    while i < args.len() {
        let a = &args[i];
        if a == "-t" || a == "--tier" {
            i += 1;
            if i < args.len() {
                tier = args[i].clone();
            }
            i += 1;
            continue;
        }
        if let Some(t) = a.strip_prefix("--tier=") {
            tier = t.to_string();
            i += 1;
            continue;
        }
        if path_opt.is_none() && !a.starts_with('-') {
            path_opt = Some(std::path::PathBuf::from(a));
        }
        i += 1;
    }
    let path = path_opt.unwrap_or(default_hdr);
    let spec = IblGenerationSpec::from_tier_name(tier.as_str());

    pollster::block_on(async move {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .expect("no adapter (GPU / pilote requis pour ce bench)");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("hdr-ibl-bench"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await
            .expect("request_device");

        let bytes = std::fs::read(&path).expect("lecture .hdr");
        let t0 = std::time::Instant::now();
        let hdr = load_hdr_from_bytes(&bytes).expect("parse HDR");
        let parse_ms = t0.elapsed().as_secs_f64() * 1e3;

        let t1 = std::time::Instant::now();
        let _ibl = IblContext::from_hdr_with_spec(&hdr, &device, &queue, &spec);
        let ibl_ms = t1.elapsed().as_secs_f64() * 1e3;
        let core = parse_ms + ibl_ms;
        eprintln!(
            "[hdr-ibl-bench] ibl_tier={} irr={} pre0={} lut={} | file={} | parse_ms={:.2} ibl_ms={:.2} core_ms={:.2}",
            tier,
            spec.irradiance_size,
            spec.prefiltered_size,
            spec.brdf_lut_size,
            path.display(),
            parse_ms,
            ibl_ms,
            core
        );
    });
}
