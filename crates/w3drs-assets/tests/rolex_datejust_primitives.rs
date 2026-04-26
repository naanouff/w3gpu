//! Nombre de primitives / profils de matériau pour `assets/models/rolex_datejust.glb` (debug affichage WASM).

use std::fs;
use std::path::PathBuf;

use w3drs_assets::{load_from_bytes, AlphaMode};

fn rolex_bytes() -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("assets/models/rolex_datejust.glb");
    fs::read(&path).unwrap_or_else(|e| panic!("lire {} : {e}", path.display()))
}

#[test]
fn rolex_datejust_is_multi_primitive() {
    let prims = load_from_bytes(&rolex_bytes()).expect("parse rolex");
    let n = prims.len();
    assert!(
        n > 1,
        "le modèle doit exposer plusieurs primitives (lunette seule = symptôme culling autre) ; n={n}"
    );
    let blend = prims
        .iter()
        .filter(|p| matches!(p.material.alpha_mode, AlphaMode::Blend))
        .count();
    assert!(blend < n, "comptes matériau cohérents: blend={blend} n={n}");
}
