//! Régression Phase A : le manifeste sous `fixtures/phases/phase-a/` reste valide et le gate GLB charge.
//!
//! `cargo test -p w3drs-assets --test phase_a_fixture`
//!
//! Si le GLB manque ou fait moins d’1 Mo : `git lfs pull` à la racine du dépôt.

use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use sha2::{Digest, Sha256};
use w3drs_assets::load_from_bytes;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn phase_a_dir() -> PathBuf {
    workspace_root().join("fixtures/phases/phase-a")
}

#[test]
fn phase_a_manifest_on_disk() {
    let dir = phase_a_dir();
    let manifest = dir.join("manifest.json");
    assert!(
        manifest.is_file(),
        "fixture Phase A attendu : {}",
        manifest.display()
    );
    let raw = fs::read_to_string(&manifest).expect("read manifest");
    assert!(
        raw.contains("phase-a"),
        "manifest inattendu (pas de phase-id) : {}",
        manifest.display()
    );
}

#[test]
fn phase_a_gate_glb_from_manifest_loads() {
    let dir = phase_a_dir();
    let manifest_path = dir.join("manifest.json");
    let raw = fs::read_to_string(&manifest_path).expect("read manifest");
    let v: Value = serde_json::from_str(&raw).expect("parse manifest.json");

    let rel = v["models"][0]["relative_path"]
        .as_str()
        .expect("manifest.models[0].relative_path");
    let glb_path = dir.join(rel);
    let glb_path = glb_path
        .canonicalize()
        .unwrap_or_else(|e| panic!("chemin GLB introuvable {} : {e}", glb_path.display()));

    let bytes = fs::read(&glb_path).unwrap_or_else(|e| {
        panic!(
            "lecture GLB échouée {} — `git lfs pull` à la racine du dépôt ? : {e}",
            glb_path.display()
        )
    });
    assert!(
        bytes.len() > 1_000_000,
        "{} ne ressemble pas au GLB complet ({} octets) — Git LFS non tiré ?",
        glb_path.display(),
        bytes.len()
    );

    let expected = v["models"][0]["expected_sha256_hex"]
        .as_str()
        .expect("manifest.models[0].expected_sha256_hex");
    let digest = format!("{:x}", Sha256::digest(&bytes));
    assert_eq!(
        digest,
        expected.to_ascii_lowercase(),
        "empreinte SHA256 du gate ne correspond pas au manifest / shortlist"
    );

    let prims = load_from_bytes(&bytes).expect("load_from_bytes gate");
    assert!(
        !prims.is_empty(),
        "le gate DamagedHelmet doit produire au moins une primitive"
    );
}
