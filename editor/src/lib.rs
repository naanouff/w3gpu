//! Coquille de l’éditeur natif w3d : shell egui, config data-driven (phase-k).

pub mod app;
pub mod assistant;
pub mod editor_config;
pub mod motor;

pub use app::{run, run_with_config, run_with_launch, EditorLaunch};
pub use editor_config::{load_editor_config_from_path, parse_editor_config_str, EditorUi, EditorUiError};
pub use w3drs_assistant_api::AssistantConfig;
