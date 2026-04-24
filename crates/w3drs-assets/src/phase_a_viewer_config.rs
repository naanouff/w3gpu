//! Config JSON **data-driven** pour les viewers Phase A (natif `khronos-pbr-sample`, **WASM** `w3drs-wasm`) :
//! paramètres globaux (IBL diffuse scale, tonemapping) par **variante**, sans recompiler.
//!
//! Fichier typique : `fixtures/phases/phase-a/materials/default.json` (version 1), reprise sous `www/public/…` pour le web.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Racine du fichier `materials/*.json` Phase A.
#[derive(Debug, Clone, Deserialize)]
pub struct PhaseAViewerConfig {
    pub version: u32,
    /// Texte libre (documentation dans le JSON) — ignoré par la logique.
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default = "default_active_variant")]
    pub active_variant: String,
    /// Variantes nommées ; la clé `active_variant` (ou `default` en repli) détermine les paramètres.
    #[serde(default)]
    pub variants: HashMap<String, PhaseAVariant>,
}

fn default_active_variant() -> String {
    "default".to_string()
}

/// Paramètres d’une variante (viewer / pipeline de tonemapping côté client de test).
#[derive(Debug, Clone, Deserialize)]
pub struct PhaseAVariant {
    #[serde(default)]
    pub label: String,
    /// Facteur appliqué à `FrameUniforms::ibl_diffuse_scale` (diffuse IBL × albedo × kd).
    #[serde(default = "default_one_f32")]
    pub ibl_diffuse_scale: f32,
    /// Résolution du bake IBL côté CPU (voir `w3drs_renderer::IblGenerationSpec::from_tier_name`) :
    /// `max` (défaut, qualité actuelle) · `high` · `medium` · `low` · `min`.
    #[serde(default = "default_ibl_tier")]
    pub ibl_tier: String,
    #[serde(default)]
    pub tonemap: Option<PhaseATonemap>,
}

fn default_one_f32() -> f32 {
    1.0
}

fn default_ibl_tier() -> String {
    "max".to_string()
}

/// Tonemapping / post (échantillon Khronos natif : exposure + bloom).
#[derive(Debug, Clone, Deserialize)]
pub struct PhaseATonemap {
    #[serde(default = "default_one_f32")]
    pub exposure: f32,
    #[serde(default)]
    pub bloom_strength: f32,
}

impl Default for PhaseAVariant {
    fn default() -> Self {
        Self {
            label: String::new(),
            ibl_diffuse_scale: 1.0,
            ibl_tier: default_ibl_tier(),
            tonemap: None,
        }
    }
}

impl Default for PhaseAViewerConfig {
    fn default() -> Self {
        Self {
            version: 1,
            comment: None,
            active_variant: "default".to_string(),
            variants: HashMap::new(),
        }
    }
}

impl PhaseAViewerConfig {
    /// Paramètres effectifs de la variante active : `active_variant` sinon `default`, sinon [`PhaseAVariant::default`].
    pub fn active_settings(&self) -> PhaseAVariant {
        self.variants
            .get(self.active_variant.as_str())
            .or_else(|| self.variants.get("default"))
            .cloned()
            .unwrap_or_default()
    }

    /// `ibl_diffuse_scale` de la variante active.
    pub fn ibl_diffuse_scale(&self) -> f32 {
        self.active_settings().ibl_diffuse_scale
    }
}

/// Parse le JSON (UTF-8) ; en cas d’échec, **warning** + [`PhaseAViewerConfig::default`]. Utilisable côté **WASM** sans `std::fs`.
pub fn parse_phase_a_viewer_config_str_or_default(json: &str) -> PhaseAViewerConfig {
    match serde_json::from_str::<PhaseAViewerConfig>(json) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Phase A viewer config: parse failed: {e}");
            PhaseAViewerConfig::default()
        }
    }
}

/// Charge le JSON depuis le disque ; en cas d’erreur de lecture ou de parse, journalise un **warning** et renvoie le défaut.
pub fn load_phase_a_viewer_config_or_default(path: &Path) -> PhaseAViewerConfig {
    let display = path.display();
    let data = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("Phase A viewer config: read failed ({display}): {e}");
            return PhaseAViewerConfig::default();
        }
    };
    match serde_json::from_str::<PhaseAViewerConfig>(&data) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Phase A viewer config: parse failed ({display}): {e}");
            PhaseAViewerConfig::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fixture_default_json() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let path = root.join("../../fixtures/phases/phase-a/materials/default.json");
        let c = load_phase_a_viewer_config_or_default(&path);
        assert_eq!(c.version, 1);
        let v = c.active_settings();
        assert!((v.ibl_diffuse_scale - 1.0).abs() < 1e-5, "{v:?}");
        assert_eq!(v.ibl_tier, "max");
    }

    #[test]
    fn parse_str_matches_fixture() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/phases/phase-a/materials/default.json");
        let s = std::fs::read_to_string(&path).expect("read fixture");
        let from_str = parse_phase_a_viewer_config_str_or_default(&s);
        let from_path = load_phase_a_viewer_config_or_default(&path);
        assert_eq!(from_str.version, from_path.version);
        assert_eq!(
            from_str.active_settings().ibl_diffuse_scale,
            from_path.ibl_diffuse_scale()
        );
    }
}
