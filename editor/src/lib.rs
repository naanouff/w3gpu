//! Coquille de l’éditeur natif w3d : shell egui, config data-driven (phase-k).

pub mod app;
pub mod assistant;
/// Application `EditProposalEnvelopeV2` → disque (transaction, *undo*).
pub mod edit_proposal_apply;
pub use edit_proposal_apply::{
    apply_proposal_v2, ApplyFilesSnapshot, ApplyRunReport, WorkspaceApplyError,
};
pub mod editor_config;
pub mod mode_rail;
pub mod motor;
pub mod v3_hifi;
pub mod viewport3d;
pub mod w3d_logo;

#[cfg(test)]
mod phase_k_workspace;

pub use app::{run, run_with_config, run_with_launch, EditorLaunch};
pub use editor_config::{load_editor_config_from_path, parse_editor_config_str, EditorUi, EditorUiError};
pub use w3drs_assistant_api::AssistantConfig;
