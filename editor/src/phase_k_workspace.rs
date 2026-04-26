//! Vérifie l’arborescence versionnée `fixtures/phases/phase-k/` (Phase K) pour les tests d’intégration.

use std::path::PathBuf;

fn phase_k_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fixtures")
        .join("phases")
        .join("phase-k")
}

#[test]
fn workspace_minimal_tree_matches_goals() {
    let w = phase_k_root().join("workspace");
    assert!(w.is_dir(), "attendu workspace/ sous phase-k: {:?}", w);
    for rel in [
        "assets",
        "src",
        "shaders",
        "dist",
        ".w3cache",
        "README.md",
        "src/default.scene.json",
        "shaders/preview.wgsl",
    ] {
        let p = w.join(rel);
        assert!(p.exists(), "manquant: {:?}", p);
    }
}

#[test]
fn extension_hello_stub_plugin_present() {
    let p = phase_k_root()
        .join("extensions")
        .join("hello_stub")
        .join("plugin.json");
    assert!(p.is_file(), "plugin.json stub: {:?}", p);
    let t = std::fs::read_to_string(&p).expect("read plugin.json");
    assert!(t.contains("w3d_plugin_hello_register"));
}

#[test]
fn expected_bake_checklist_file_exists() {
    let p = phase_k_root().join("expected.md");
    assert!(p.is_file(), "{:?}", p);
}
