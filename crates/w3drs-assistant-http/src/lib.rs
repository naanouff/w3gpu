//! Client HTTP Ollama (`/api/chat`, flux NDJSON) — s’exécute dans un **std::thread** (non-bloquant pour l’UI).

mod ollama;

pub use ollama::OllamaBackend;
