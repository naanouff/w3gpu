//! Assistant IA w3d : **aucune** dépendance moteur ; le shell éditeur peut l’inclure avec une impl
//! **Noop** par défaut et une option **sidecar** (HTTP) dans un crate à part.

mod apply_v2;
mod config;
mod edit_proposal_v2;
mod service;

pub use apply_v2::{apply_one_to_utf8, merge_json_rfc7396_in_place, EditApplyError};
pub use config::{
    load_assistant_config_from_path, parse_assistant_config_str, AssistantConfig, AssistantConfigError,
};
pub use edit_proposal_v2::{
    ConfigSchemaIdV2, EditFileReplaceV2, EditOpV2, EditProposalEnvelopeV2, EditProposalValidationError, ReplaceRangeV2,
};
pub use service::{
    read_workspace_assistant_path_hint, try_recv_for, ActiveStream, AssistantBackend, AssistantError, AssistantEvent,
    ChatMessage, collect_tokens_simple, CompletionRequest, NoopBackend,
};
