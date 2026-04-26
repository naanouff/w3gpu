//! Schéma `editor/assistant.json` (ou copie phase-k) : **désactivé** par défaut, data-driven.

use std::path::Path;

use serde::Deserialize;
use thiserror::Error;

/// Configuration assistant (fichier JSON, camelCase) — `enabled: false` par défaut requis en prod.
#[derive(Debug, Clone, PartialEq)]
pub struct AssistantConfig {
    pub version: u8,
    /// Si `false`, le shell n’ouvre **aucun** appel réseau ni thread d’inférence (comportement Noop).
    pub enabled: bool,
    /// Ex. Ollama : `http://127.0.0.1:11434`
    pub base_url: String,
    pub model: String,
    pub temperature: f32,
    /// Limite côté client (requête HTTP).
    pub request_timeout_sec: u64,
}

impl Default for AssistantConfig {
    fn default() -> Self {
        Self {
            version: 1,
            enabled: false,
            base_url: "http://127.0.0.1:11434".to_string(),
            model: "llama3.2".to_string(),
            temperature: 0.7,
            request_timeout_sec: 120,
        }
    }
}

#[derive(Debug, Deserialize)]
struct FileConfig {
    version: u8,
    enabled: bool,
    #[serde(rename = "baseUrl")]
    base_url: String,
    model: String,
    temperature: f32,
    #[serde(rename = "requestTimeoutSec")]
    request_timeout_sec: u64,
}

#[derive(Debug, Error)]
pub enum AssistantConfigError {
    #[error("assistant config: JSON: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("assistant config: {0}")]
    Invalid(String),
}

/// Lit un fichier UTF-8 (phase-k ou `editor/assistant.json` d’un workspace).
pub fn load_assistant_config_from_path(path: &Path) -> Result<AssistantConfig, AssistantConfigError> {
    let s = std::fs::read_to_string(path)
        .map_err(|e| AssistantConfigError::Invalid(format!("lecture {}: {e}", path.display())))?;
    parse_assistant_config_str(&s)
}

/// Parse + validation légère (même esprit que `EditorUi` phase-k).
pub fn parse_assistant_config_str(s: &str) -> Result<AssistantConfig, AssistantConfigError> {
    let f: FileConfig = serde_json::from_str(s)?;
    if f.version != 1 {
        return Err(AssistantConfigError::Invalid(format!(
            "version 1 requise, reçu {}",
            f.version
        )));
    }
    if f.base_url.trim().is_empty() {
        return Err(AssistantConfigError::Invalid("baseUrl vide".into()));
    }
    if f.model.trim().is_empty() {
        return Err(AssistantConfigError::Invalid("model vide".into()));
    }
    if !f.temperature.is_finite() || f.temperature < 0.0 || f.temperature > 2.0 {
        return Err(AssistantConfigError::Invalid(format!(
            "temperature hors plage [0, 2] / non fini: {}",
            f.temperature
        )));
    }
    if f.request_timeout_sec == 0 || f.request_timeout_sec > 3_600 {
        return Err(AssistantConfigError::Invalid(format!(
            "requestTimeoutSec plage 1..=3600, reçu {}",
            f.request_timeout_sec
        )));
    }
    Ok(AssistantConfig {
        version: 1,
        enabled: f.enabled,
        base_url: f.base_url,
        model: f.model,
        temperature: f.temperature,
        request_timeout_sec: f.request_timeout_sec,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN_OK: &str = r#"{
  "version": 1,
  "enabled": false,
  "baseUrl": "http://127.0.0.1:11434",
  "model": "llama3.2",
  "temperature": 0.7,
  "requestTimeoutSec": 120
}"#;

    #[test]
    fn parse_fixture_shape() {
        let c = parse_assistant_config_str(MIN_OK).expect("ok");
        assert!(!c.enabled);
        assert_eq!(c.base_url, "http://127.0.0.1:11434");
        assert_eq!(c.model, "llama3.2");
        assert_eq!(c.temperature, 0.7);
    }

    #[test]
    fn err_bad_version() {
        let mut v: serde_json::Value = serde_json::from_str(MIN_OK).unwrap();
        *v.get_mut("version").unwrap() = serde_json::json!(2);
        let bad = v.to_string();
        assert!(parse_assistant_config_str(&bad).is_err());
    }

    #[test]
    fn err_temperature() {
        let mut v: serde_json::Value = serde_json::from_str(MIN_OK).unwrap();
        *v.get_mut("temperature").unwrap() = serde_json::json!(-0.1);
        assert!(parse_assistant_config_str(&v.to_string()).is_err());
    }

    #[test]
    fn err_timeout_zero() {
        let mut v: serde_json::Value = serde_json::from_str(MIN_OK).unwrap();
        *v.get_mut("requestTimeoutSec").unwrap() = serde_json::json!(0);
        assert!(parse_assistant_config_str(&v.to_string()).is_err());
    }

    #[test]
    fn default_config_matches() {
        let c = parse_assistant_config_str(MIN_OK).unwrap();
        let d = AssistantConfig {
            version: 1,
            ..AssistantConfig::default()
        };
        // Default model/base match fixture defaults for doc parity; enabled stays false
        assert_eq!(c.model, d.model);
        assert_eq!(c.base_url, d.base_url);
    }
}
