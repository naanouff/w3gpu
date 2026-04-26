//! Exemple **Phase B** : charge [`render_graph.json`](../../fixtures/phases/phase-b/render_graph.json),
//! valide (`validate_render_graph_exec_v0`) puis exécute une frame **`run_graph_v0_checksum`**
//! (même logique que `cargo test -p w3drs-renderer --test phase_b_graph_exec`).
//!
//! ## Usage
//!
//! ```text
//! cargo run -p phase-b-graph --release
//! ```
//!
//! Sans argument, le binaire pointe le fixture du dépôt : `fixtures/phases/phase-b/`.
//!
//! Avec un **chemin absolu ou relatif existant** (répertoire contenant `render_graph.json` et `shaders/`) :
//!
//! ```text
//! cargo run -p phase-b-graph --release -- "D:\projets\w3drs\fixtures\phases\phase-b"
//! ```
//!
//! Ne pas copier un **placeholder** de doc (`C:\chemin\vers\…`) : le dossier doit vraiment exister.
//!
//! **Aucune fenêtre** n’apparaît : c’est un outil en ligne de commande (une seule frame GPU, puis
//! message sur la console et fin — pas un viewer graphique).

use std::io::Write;
use std::path::PathBuf;

use w3drs_renderer::{
    parse_render_graph_json, run_graph_v0_checksum, validate_render_graph_exec_v0,
};

fn default_phase_b_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("phases")
        .join("phase-b")
}

fn try_gpu() -> Option<(wgpu::Device, wgpu::Queue)> {
    pollster::block_on(async {
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
            .ok()?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("phase-b-graph"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::default(),
                ..Default::default()
            })
            .await
            .ok()?;
        Some((device, queue))
    })
}

fn main() {
    let _ = writeln!(
        std::io::stdout(),
        "phase-b-graph — démo headless (pas de fenêtre) : chargement du graphe, 1 frame GPU, checksum, puis fin.\n"
    );
    let args: Vec<String> = std::env::args().skip(1).collect();
    let from_cli: Option<PathBuf> = args.get(0).map(|s| PathBuf::from(s));
    if let Some(ref p) = from_cli {
        if !p.is_dir() {
            eprintln!(
                "[phase-b-graph] le répertoire n’existe pas : {}\n  → indiquez un chemin valide, ou omettez l’argument pour le fixture intégré (fixtures/phases/phase-b).",
                p.display()
            );
            std::process::exit(1);
        }
    }
    let a_passé_chemin_explicite = from_cli.is_some();
    let root = from_cli.unwrap_or_else(default_phase_b_root);
    let json_path = root.join("render_graph.json");
    let readback_id = "hdr_color";

    let json = std::fs::read_to_string(&json_path).unwrap_or_else(|e| {
        eprintln!("[phase-b-graph] lecture {} : {e}", json_path.display());
        eprintln!(
            "  → vérifier que le dossier contient render_graph.json et shaders/ ; ne pas utiliser un chemin « exemple » de tutoriel s’il n’existe pas sur la machine."
        );
        if a_passé_chemin_explicite {
            eprintln!(
                "  → relancer **sans** argument en étant à la racine w3drs : `cargo run -p phase-b-graph --release`"
            );
        }
        std::process::exit(1);
    });
    let doc = parse_render_graph_json(&json).unwrap_or_else(|e| {
        eprintln!("[phase-b-graph] parse JSON : {e}");
        std::process::exit(1);
    });
    if let Err(e) = validate_render_graph_exec_v0(&doc, readback_id) {
        eprintln!("[phase-b-graph] validate : {e}");
        std::process::exit(1);
    }
    let Some((device, queue)) = try_gpu() else {
        eprintln!(
            "[phase-b-graph] pas d’adaptateur GPU (pilote / machine requis pour le checksum)."
        );
        let _ = writeln!(
            std::io::stdout(),
            "ÉCHEC : aucun adaptateur WebGPU (wgpu) — pas de checksum. \
             Vérifiez pilotes graphiques, ou exécutez sur une machine avec GPU.\n"
        );
        std::process::exit(1);
    };
    let _ = writeln!(std::io::stdout(), "GPU OK — exécution du graphe…");
    let _ = std::io::stdout().flush();
    let checksum =
        run_graph_v0_checksum(&device, &queue, &doc, &root, readback_id).unwrap_or_else(|e| {
            eprintln!("[phase-b-graph] exécution : {e}");
            let _ = writeln!(std::io::stdout(), "ÉCHEC exécution : {e}\n");
            std::process::exit(1);
        });
    let out = format!(
        "\nRésultat — readback={}  checksum (FNV-1a 64) = {}\nroot = {}\n\nTerminé (processus quitté — c’est normal, il n’y a pas d’UI).\n",
        readback_id, checksum, root.display()
    );
    print!("{out}");
    eprintln!(
        "[phase-b-graph] readback={} checksum={} root={}",
        readback_id,
        checksum,
        root.display()
    );
}
