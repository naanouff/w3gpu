//! `AssistantBackend` + streaming événementiel, impl **Noop** pour chemins d’UI sans option sidecar.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use thiserror::Error;

/// Message d’entretien type chat (rôles alphanum, attendus : `system` / `user` / `assistant` côté modèle).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Requête d’achèvement; `model` peut surcharger la config.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CompletionRequest {
    pub model_override: Option<String>,
    pub temperature: Option<f32>,
    pub messages: Vec<ChatMessage>,
}

/// Événement diffusé vers l’UI (ou consommateur) — un flux par requête.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssistantEvent {
    /// Fragment de texte (delta).
    Token(String),
    /// Réponse Ollama / outil a terminé sans texte d’erreur côté protocole.
    Done,
    /// Erreur côté transport, parse, ou règles sûres.
    Error(AssistantError),
}

/// Erreur non fatale pour l’exécutable (l’UI peut journaliser continuer).
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum AssistantError {
    #[error("assistant: désactivé (config)")]
    Disabled,
    #[error("assistant: requête invalide: {0}")]
    BadRequest(&'static str),
    #[error("assistant: transport: {0}")]
    Transport(String),
    #[error("assistant: parse réponse: {0}")]
    ParseResponse(String),
    #[error("assistant: annulé par l’utilisateur")]
    Aborted,
    #[error("assistant: I/O: {0}")]
    Io(String),
}

/// Contrôle d’un flux d’événements; **annulable** sans tuer l’hôte.
pub struct ActiveStream {
    /// Récepteur côté UI; si droppé, le fil de travail se termine dès la prochaine tentative d’`send`.
    pub events: mpsc::Receiver<AssistantEvent>,
    cancel: Arc<AtomicBool>,
    worker: thread::JoinHandle<()>,
}

impl ActiveStream {
    /// Construit un flux actif (utilisé par **w3drs-assistant-http** et les backends tiers).
    pub fn from_worker(
        events: mpsc::Receiver<AssistantEvent>,
        cancel: Arc<AtomicBool>,
        worker: thread::JoinHandle<()>,
    ) -> Self {
        Self { events, cancel, worker }
    }
    /// Signale l’abandon; le consommateur reçoit éventuellement `Error(Aborted)`.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    /// Attend la fin du fil (après `cancel` optionnelle).
    pub fn join(self) -> thread::Result<()> {
        self.worker.join()
    }
}

/// Backend minimal : une implémentation par transport (Noop, HTTP, futur in-process).
pub trait AssistantBackend: Send + Sync {
    /// Démarre un achèvement en fil de fond. Ne doit **pas** bloquer le fil UI.
    fn start_completion(&self, req: CompletionRequest) -> Result<ActiveStream, AssistantError>;
}

/// Aucun appel réseau, aucun texte; répond `Done` immédiatement.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopBackend;

impl AssistantBackend for NoopBackend {
    fn start_completion(&self, req: CompletionRequest) -> Result<ActiveStream, AssistantError> {
        if !req.messages.is_empty() {
            for m in &req.messages {
                if m.content.len() > 1_000_000 {
                    return Err(AssistantError::BadRequest("contenu d’un message trop long"));
                }
            }
        }
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let c = cancel.clone();
        let h = thread::spawn(move || {
            if c.load(Ordering::SeqCst) {
                let _ = tx.send(AssistantEvent::Error(AssistantError::Aborted));
                return;
            }
            let _ = tx.send(AssistantEvent::Done);
        });
        Ok(ActiveStream::from_worker(rx, cancel, h))
    }
}

/// Durée d’attente côté UI pour vider le canal sans spin (tests / greffiers). Export public ; tests dans ce module.
pub fn try_recv_for(rx: &mpsc::Receiver<AssistantEvent>, timeout: Duration) -> Option<AssistantEvent> {
    rx.recv_timeout(timeout).ok()
}

/// Aide tests : reçoit tous les évènement jusqu’à `Done` ou `Error` (sauf Token vides agglomérés côté appelant).
pub fn collect_tokens_simple(
    rx: &mpsc::Receiver<AssistantEvent>,
    deadline: Duration,
) -> Result<String, AssistantError> {
    let end = std::time::Instant::now() + deadline;
    let mut s = String::new();
    loop {
        let left = end.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            return Err(AssistantError::Transport("délai d’essai collect_tokens_simple".into()));
        }
        match try_recv_for(rx, left) {
            Some(AssistantEvent::Token(t)) => s.push_str(&t),
            Some(AssistantEvent::Done) => return Ok(s),
            Some(AssistantEvent::Error(e)) => return Err(e),
            None => {
                return Err(AssistantError::Transport("aucun événement avant timeout".into()));
            }
        }
    }
}

/// Placeholder (aucune logique) pour alignement futur *workspace* : l’hôte lira
/// un fichier disque; l’API ne connaît pas l’`std::fs` ici, hormis le signature path.
pub fn read_workspace_assistant_path_hint(base: &Path) -> std::path::PathBuf {
    base.join("editor").join("assistant.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_produces_done() {
        let b = NoopBackend;
        let st = b.start_completion(CompletionRequest::default()).expect("ok");
        match st.events.recv() {
            Ok(AssistantEvent::Done) => {}
            e => panic!("unexpected {e:?}"),
        }
    }

    #[test]
    fn noop_rejects_huge_message() {
        let b = NoopBackend;
        let huge = "x".repeat(1_200_000);
        let e = b
            .start_completion(CompletionRequest {
                messages: vec![ChatMessage {
                    role: "user".to_string(),
                    content: huge,
                }],
                ..Default::default()
            })
            .err()
            .expect("err");
        assert!(matches!(e, AssistantError::BadRequest(_)));
    }

    #[test]
    fn collect_tokens_simple_on_noop() {
        let b = NoopBackend;
        let s = b.start_completion(CompletionRequest::default()).unwrap();
        let t = std::time::Duration::from_secs(2);
        let r = super::collect_tokens_simple(&s.events, t);
        assert_eq!(r, Ok(String::new()));
        let _ = s.join();
    }

    #[test]
    fn try_recv_for_may_timeout() {
        use std::sync::mpsc;
        let (tx, rx) = mpsc::channel();
        let _ = tx.send(AssistantEvent::Token("a".into()));
        let got = try_recv_for(&rx, std::time::Duration::from_secs(1));
        assert_eq!(got, Some(AssistantEvent::Token("a".into())));
    }

    #[test]
    fn cancel_before_recv_may_abort_or_done() {
        // Course possible : le Noop enfile Done très vite, donc on se contente de ne pas paniquer
        // si cancel tôt.
        let b = NoopBackend;
        let st = b.start_completion(CompletionRequest::default()).unwrap();
        st.cancel();
        let _ = st.join();
    }

    #[test]
    fn read_workspace_hint() {
        let p = read_workspace_assistant_path_hint(std::path::Path::new("/w"));
        assert_eq!(p.file_name().and_then(|f| f.to_str()), Some("assistant.json"));
        let parent = p.parent().expect("parent");
        assert_eq!(parent.file_name().and_then(|f| f.to_str()), Some("editor"));
    }
}
