use std::io::BufRead;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use serde::Serialize;
use w3drs_assistant_api::{ActiveStream, AssistantBackend, AssistantError, AssistantEvent, CompletionRequest};

use w3drs_assistant_api::AssistantConfig;

/// Backend basé sur `POST {baseUrl}/api/chat` (format Ollama) avec `stream: true`.
#[derive(Debug, Clone)]
pub struct OllamaBackend {
    cfg: Arc<AssistantConfig>,
}

impl OllamaBackend {
    /// Valide l’URL de base.
    pub fn from_config(cfg: &AssistantConfig) -> Result<Self, String> {
        let l = cfg.base_url.to_lowercase();
        if !l.starts_with("http://") && !l.starts_with("https://") {
            return Err("baseUrl doit commencer par http:// ou https://".into());
        }
        Ok(Self { cfg: Arc::new(cfg.clone()) })
    }
}

impl AssistantBackend for OllamaBackend {
    fn start_completion(&self, req: CompletionRequest) -> Result<ActiveStream, AssistantError> {
        for m in &req.messages {
            if m.content.is_empty() {
                return Err(AssistantError::BadRequest("un message a un contenu vide"));
            }
            if m.content.len() > 2_000_000 {
                return Err(AssistantError::BadRequest("contenu d’un message trop long"));
            }
        }
        if req.messages.is_empty() {
            return Err(AssistantError::BadRequest("au moins un message requis"));
        }

        let cfg = self.cfg.clone();
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let c = cancel.clone();

        let h = thread::spawn(move || {
            run_ollama_stream(&cfg, &req, c, &tx);
        });

        Ok(ActiveStream::from_worker(rx, cancel, h))
    }
}

#[derive(Serialize)]
struct OllamaChatRequest<'a> {
    model: String,
    messages: Vec<OllamaMessage<'a>>,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct OllamaMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
}

#[derive(Deserialize, Debug)]
struct OllamaStreamLine {
    /// Présent sur toutes les lignes utiles ; absent sur la ligne finale `{"done":true}` de certains serveurs.
    message: Option<OllamaStreamMessage>,
    #[serde(default)]
    done: bool,
}

#[derive(Deserialize, Debug)]
struct OllamaStreamMessage {
    content: String,
}

fn run_ollama_stream(
    cfg: &AssistantConfig,
    req: &CompletionRequest,
    cancel: Arc<AtomicBool>,
    tx: &mpsc::Sender<AssistantEvent>,
) {
    let url = format!("{}/api/chat", cfg.base_url.trim_end_matches('/'));
    let model = match &req.model_override {
        Some(m) if !m.is_empty() => m.clone(),
        _ => cfg.model.clone(),
    };
    let t = match req.temperature {
        Some(x) if x.is_finite() && (0.0..=2.0).contains(&x) => x,
        _ => cfg.temperature,
    };

    let ollama_msgs: Vec<OllamaMessage> = req
        .messages
        .iter()
        .map(|m| OllamaMessage {
            role: m.role.as_str(),
            content: m.content.as_str(),
        })
        .collect();

    let body = OllamaChatRequest {
        model,
        messages: ollama_msgs,
        stream: true,
        options: OllamaOptions { temperature: t },
    };

    let body_json = match serde_json::to_string(&body) {
        Ok(s) => s,
        Err(e) => {
            let _ = tx.send(AssistantEvent::Error(AssistantError::Transport(format!("json {e}"))));
            return;
        }
    };

    if cancel.load(Ordering::SeqCst) {
        let _ = tx.send(AssistantEvent::Error(AssistantError::Aborted));
        return;
    }

    let resp = match ureq::post(&url)
        .timeout(Duration::from_secs(cfg.request_timeout_sec))
        .set("Content-Type", "application/json; charset=utf-8")
        .send_string(&body_json)
    {
        Ok(r) => r,
        Err(e) => {
            let _ = tx.send(AssistantEvent::Error(AssistantError::Transport(format!("{e}"))));
            return;
        }
    };

    let code = resp.status();
    if code < 200 || code >= 300 {
        let err_body = match resp.into_string() {
            Ok(s) => s,
            Err(e) => format!("<lecture Corps: {e}>"),
        };
        let _ = tx.send(AssistantEvent::Error(AssistantError::Transport(format!(
            "status HTTP {code}: {}",
            err_body.chars().take(200).collect::<String>()
        ))));
        return;
    }

    let mut reader = std::io::BufReader::new(resp.into_reader());
    let mut line = String::new();
    loop {
        if cancel.load(Ordering::SeqCst) {
            let _ = tx.send(AssistantEvent::Error(AssistantError::Aborted));
            return;
        }
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => {
                let _ = tx.send(AssistantEvent::Done);
                return;
            }
            Ok(_) => {
                let t = line.trim();
                if t.is_empty() {
                    continue;
                }
                let v: OllamaStreamLine = match serde_json::from_str(t) {
                    Ok(x) => x,
                    Err(e) => {
                        let _ = tx.send(AssistantEvent::Error(AssistantError::ParseResponse(format!("{e}: {t}"))));
                        return;
                    }
                };
                if let Some(m) = v.message {
                    if !m.content.is_empty() && tx.send(AssistantEvent::Token(m.content)).is_err() {
                        return;
                    }
                }
                if v.done {
                    let _ = tx.send(AssistantEvent::Done);
                    return;
                }
            }
            Err(e) => {
                let _ = tx.send(AssistantEvent::Error(AssistantError::Io(e.to_string())));
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(t: &str) -> OllamaStreamLine {
        serde_json::from_str(t).expect("ok")
    }

    #[test]
    fn ollama_line_with_content() {
        let t = r#"{"model":"x","message":{"content":"h"},"done":false}"#;
        let p = parse(t);
        assert_eq!(p.message.map(|m| m.content), Some("h".into()));
    }

    #[test]
    fn ollama_line_done_no_message() {
        let t = r#"{"done":true}"#;
        let p: OllamaStreamLine = serde_json::from_str(t).expect("ok");
        assert!(p.message.is_none());
    }
}
