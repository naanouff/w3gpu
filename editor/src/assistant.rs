//! Chargement `assistant.json` + choix de backend (Noop / Ollama **HTTP** derrière la feature `sidecar-ollama`).

use std::path::Path;
use std::sync::Arc;

use w3drs_assistant_api::{
    load_assistant_config_from_path, ActiveStream, AssistantBackend, AssistantConfig, AssistantError, AssistantEvent,
    ChatMessage, CompletionRequest, NoopBackend,
};

use w3drs_assistant_api::read_workspace_assistant_path_hint;

/// Même répertoire que `editor-ui.json` : `assistant.json` optionnel.
pub fn assistant_config_path_next_to_editor_config(editor_config_path: &Path) -> std::path::PathBuf {
    let parent = editor_config_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    parent.join("assistant.json")
}

/// Charge l’assistant si le fichier existe ; sinon **défaut** (désactivé) sans erreur.
pub fn try_load_assistant_config(editor_config_path: &Path) -> AssistantConfig {
    let p = assistant_config_path_next_to_editor_config(editor_config_path);
    if p.is_file() {
        return load_assistant_config_from_path(&p).unwrap_or_else(|e| {
            eprintln!("assistant.json: {e} — prudence, fallback Noop / défaut");
            AssistantConfig::default()
        });
    }
    AssistantConfig::default()
}

/// Construit le backend : `Ollama` seulement si `enabled` **et** feature `sidecar-ollama` ; sinon `Noop`.
pub fn make_assistant_backend(
    cfg: &AssistantConfig,
) -> (Arc<dyn AssistantBackend + Send + Sync>, String) {
    if !cfg.enabled {
        return (
            Arc::new(NoopBackend),
            "désactivé (assistant.json)".to_string(),
        );
    }
    #[cfg(feature = "sidecar-ollama")]
    {
        match w3drs_assistant_http::OllamaBackend::from_config(cfg) {
            Ok(b) => (Arc::new(b), "Ollama (HTTP) — requêtes vers baseUrl".to_string()),
            Err(e) => {
                eprintln!("Ollama backend: {e}");
                (
                    Arc::new(NoopBackend),
                    format!("erreur init Ollama, fallback Noop: {e}"),
                )
            }
        }
    }
    #[cfg(not(feature = "sidecar-ollama"))]
    {
        (
            Arc::new(NoopBackend),
            "recompiler w3d-editor avec --features sidecar-ollama pour le client HTTP Ollama"
                .to_string(),
        )
    }
}

/// Raccourci : chemin logique d’un **workspace** (doc / alignement futur `Goals.md`).
pub fn workspace_assistant_path(base: &Path) -> std::path::PathBuf {
    read_workspace_assistant_path_hint(base)
}

/// Un tour de réception d’événements (UI : appeler chaque frame).
pub fn drain_assistant_events(
    stream: &ActiveStream,
) -> (Vec<AssistantEvent>, Option<AssistantError>) {
    let mut v = Vec::new();
    let mut err = None;
    while let Ok(e) = stream.events.try_recv() {
        if let AssistantEvent::Error(ref er) = e {
            err = Some(er.clone());
        }
        v.push(e);
    }
    (v, err)
}

/// Prépare une requête **texte** simple (V1) — V2 = propositions JSON, non appliquées ici.
pub fn build_user_completion(model_override: Option<String>, user_text: &str) -> CompletionRequest {
    CompletionRequest {
        model_override,
        temperature: None,
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: user_text.to_string(),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_to_config_path() {
        let p = std::path::PathBuf::from("C:/proj/editor-ui.json");
        let a = assistant_config_path_next_to_editor_config(&p);
        assert!(a.to_string_lossy().ends_with("assistant.json"));
    }

    #[test]
    fn parse_default_str() {
        use w3drs_assistant_api::parse_assistant_config_str;
        let s = r#"{"version":1,"enabled":false,"baseUrl":"http://x","model":"m","temperature":0.5,"requestTimeoutSec":30}"#;
        let c = parse_assistant_config_str(s).unwrap();
        assert!(!c.enabled);
    }

    #[test]
    fn public_workspace_hint_uses_api() {
        let p = super::workspace_assistant_path(std::path::Path::new("C:/w"));
        assert_eq!(p.file_name().and_then(|f| f.to_str()), Some("assistant.json"));
    }
}
