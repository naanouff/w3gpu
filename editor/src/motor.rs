//! Raccordement des **données** moteur (sans wgpu ici) : même chemins versionnés que
//! `khronos-pbr-sample` / WASM — prérequis à un futur rendu wgpu dans le panneau central.
//!
//! Le **rendu 3D** (swapchain, `w3drs-renderer`) sera branché en **eframe wgpu** + unification
//! des versions `wgpu` / `windows` (voir `README`).

use std::path::PathBuf;

use w3drs_assets::{load_phase_a_viewer_config_or_default, PhaseAViewerConfig};

/// Config Phase A lue sur disque (même `default.json` que le viewer Khronos / `www/public`).
#[derive(Debug, Clone)]
pub struct EngineBootstrap {
    /// JSON Phase A (variantes, tonemapping, IBL tier, …).
    pub phase_a: PhaseAViewerConfig,
    pub source_path: PathBuf,
}

/// Depuis le dossier `editor/`, le dépôt est le parent.
pub fn default_phase_a_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fixtures")
        .join("phases")
        .join("phase-a")
        .join("materials")
        .join("default.json")
}

/// Charge le fichier fixture ; repli sûr identique `khronos` / `load_phase_a_viewer_config_or_default`.
pub fn load_engine_bootstrap() -> EngineBootstrap {
    let p = default_phase_a_path();
    let phase_a = load_phase_a_viewer_config_or_default(&p);
    EngineBootstrap {
        phase_a,
        source_path: p,
    }
}

/// Résumé affichage UI (1 ligne) pour la variante active.
pub fn engine_status_line(b: &EngineBootstrap) -> String {
    let v = b.phase_a.active_settings();
    let t = b.phase_a.active_variant.as_str();
    let exp = v
        .tonemap
        .as_ref()
        .map(|x| x.exposure)
        .unwrap_or(1.0);
    format!(
        "Moteur (données) : Phase A v{} · variant={t} · IBL={} · exp={exp:.2}",
        b.phase_a.version,
        v.ibl_tier
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_phase_a_path_points_at_fixture() {
        let p = default_phase_a_path();
        let lossy = p.to_string_lossy();
        assert!(
            lossy.contains("phase-a") && lossy.contains("default.json"),
            "attendu un chemin fixture phase-a: {p:?}"
        );
        assert!(
            p.is_file(),
            "le fixture Phase A doit exister à {p:?} (clone dépôt complet, CI / dev local)"
        );
    }

    #[test]
    fn load_fixture_phase_a() {
        let b = load_engine_bootstrap();
        assert!(b.phase_a.version >= 1);
        assert!(!b.phase_a.active_variant.is_empty());
    }

    #[test]
    fn status_line_non_vide() {
        let b = load_engine_bootstrap();
        let s = engine_status_line(&b);
        assert!(s.contains("Phase A"));
    }
}
