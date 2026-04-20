//! Vérifie que `KHR_materials_anisotropy` est lu depuis le GLB AnisotropyBarnLamp.

use std::fs;
use std::path::PathBuf;

use w3drs_assets::load_from_bytes;

fn barn_lamp_bytes() -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("fixtures/phases/phase-a/glb/AnisotropyBarnLamp.glb");
    fs::read(&path).unwrap_or_else(|e| panic!("lire {} : {e}", path.display()))
}

#[test]
fn anisotropy_barn_lamp_exposes_extension_on_some_primitive() {
    let prims = load_from_bytes(&barn_lamp_bytes()).expect("parse");
    let max_strength = prims
        .iter()
        .map(|p| p.material.anisotropy_strength)
        .fold(0.0f32, f32::max);
    assert!(
        max_strength > 0.01,
        "attendu au moins un matériau avec anisotropyStrength > 0, max={max_strength}"
    );
}
