//! Régression Phase A : le manifeste sous `fixtures/phases/phase-a/` reste valide et chaque GLB listé charge.
//!
//! `cargo test -p w3drs-assets --test phase_a_fixture`
//!
//! Si un `.glb` attendu est **très petit** (pointeur Git LFS non matérialisé) ou absent : `git lfs pull` à la racine du dépôt.
//! Le gate **DamagedHelmet** fait ~1 Mo une fois résolu ; les autres entrées `models[]` sous `glb/` sont aussi en **Git LFS** (y compris les micro-fixtures Khronos empaquetées, ex. TextureTransformTest ~27 Ko).

use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use sha2::{Digest, Sha256};
use w3drs_assets::load_from_bytes;

/// Seuil minimal (octets) pour rejeter un pointeur Git LFS non résolu (~130 o) ou un fichier vide.
/// Reste bien en dessous des gates lourdes (casque, lampe, etc.) mais autorise des micro-fixtures
/// Khronos empaquetées en `.glb` (ex. `TextureTransformTest`, ~27 Ko).
const MIN_GLB_BYTES: u64 = 4_096;

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
fn phase_a_all_manifest_models_load() {
    let dir = phase_a_dir();
    let manifest_path = dir.join("manifest.json");
    let raw = fs::read_to_string(&manifest_path).expect("read manifest");
    let v: Value = serde_json::from_str(&raw).expect("parse manifest.json");
    let models = v["models"]
        .as_array()
        .expect("manifest.models must be a non-empty array");
    assert!(
        !models.is_empty(),
        "manifest.models doit contenir au moins un modèle"
    );

    for (i, m) in models.iter().enumerate() {
        let id = m["id"].as_str().unwrap_or("<missing id>");
        let rel = m["relative_path"]
            .as_str()
            .unwrap_or_else(|| panic!("manifest.models[{i}].relative_path ({id})"));
        let glb_path = dir.join(rel);
        let glb_path = glb_path.canonicalize().unwrap_or_else(|e| {
            panic!(
                "chemin GLB introuvable {} (modèle {id}) : {e}",
                glb_path.display()
            )
        });

        let bytes = fs::read(&glb_path).unwrap_or_else(|e| {
            panic!(
                "lecture GLB échouée {} (modèle {id}) — `git lfs pull` si LFS ? : {e}",
                glb_path.display()
            )
        });
        assert!(
            bytes.len() as u64 >= MIN_GLB_BYTES,
            "{} ({id}) : {} octets — fichier trop petit (pointeur LFS ou manquant ?)",
            glb_path.display(),
            bytes.len(),
        );

        let expected = m["expected_sha256_hex"]
            .as_str()
            .unwrap_or_else(|| panic!("manifest.models[{i}].expected_sha256_hex ({id})"));
        let digest = format!("{:x}", Sha256::digest(&bytes));
        assert_eq!(
            digest,
            expected.to_ascii_lowercase(),
            "SHA256 modèle {id} ne correspond pas au manifest"
        );

        let prims =
            load_from_bytes(&bytes).unwrap_or_else(|e| panic!("load_from_bytes({id}) : {e}"));
        assert!(
            !prims.is_empty(),
            "modèle {id} doit produire au moins une primitive"
        );
    }
}
