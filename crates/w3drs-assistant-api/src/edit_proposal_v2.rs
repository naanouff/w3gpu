//! **Phase V2** (plan produit) : l’UI **ne décide** pas seule d’appliquer des edits; le moteur / shell
//! valide et l’auteur **Accept / Reject** (diff) avant toute écriture disque.
//!
//! # Flux
//!
//! 1. L’utilisateur (ou l’assitant) génère un `EditProposalEnvelopeV2` JSON (p.ex. extrait d’un
//!    bloc de réponse outil, ou d’un appel de contrat côté serveur).
//! 2. L’éditeur **parse** et **valide** (`path` relatif, pas d’`..`, tailles raisonnables).
//! 3. Aucun `fs::write` tant que l’utilisateur n’a pas cliqué **Appliquer** (équivalent Copilot : rejectable).
//! 4. L’**application** reste le code w3d (même module que l’onboarding workspace), pas l’appelant LLM.
//!
//! Exemple JSON : [`../../fixtures/phases/phase-k/assistant-edit-proposal-v2.example.json`](../../fixtures/phases/phase-k/assistant-edit-proposal-v2.example.json)

use serde::{Deserialize, Serialize};

/// Catégorie sémantique (contrôle sûr de la cible configuration).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigSchemaIdV2 {
    /// `editor/editor-ui.json` (shell, modes) — *data-driven* UI.
    EditorShell,
    /// Fichier scène `src/…/….json` (hors binaire `.w3db`).
    Scene,
    /// `editor/assistant.json` (Ollama / no-op).
    Assistant,
    /// Autre JSON texte dans les préfixes workspace autorisés.
    GenericJson,
}

/// Erreur de **validation côté éditeur** (hors sémantique de fichiers : pas d’E/S ici).
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum EditProposalValidationError {
    /// Enveloppe mal formée.
    #[error("enveloppe / op corrompue: {0}")]
    Corrupt(&'static str),
    /// Chemin interdit (`..`, absolu, vide).
    #[error("chemin cible interdit (relatif, sans `..`)")]
    BadPath,
    /// Texte d’opération dépassant un plafond sûr (anti bomb).
    #[error("champ {field} dépasse le plafond: max {max} octets, reçu {got}")]
    ExceedsSizeLimit { field: &'static str, max: usize, got: usize },
}

const MAX_PATH: usize = 1_024;
const MAX_REPLACEMENT: usize = 1_000_000;
const MAX_ID: usize = 64;
const MAX_SUMMARY: usize = 8_192;
const MAX_OPS: usize = 1_000;
/// Taille max sérialisée du document « patch » (merge JSON) pour `configJsonMergePatch`.
const MAX_JSON_PATCH_VALUE: usize = 256_000;

/// Enveloppe versionnée; `version` doit être 2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditProposalEnvelopeV2 {
    pub version: u8,
    /// Identifiant d’idempotence / suivi d’UI (p.ex. UUID court).
    pub id: String,
    pub summary: String,
    pub ops: Vec<EditOpV2>,
}

/// Opération ciblée, extensible par `op` tag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum EditOpV2 {
    /// Remplace toutes les occurrences de `search` par `replace` (comportement de base).
    /// Pour des retraits fins, utiliser plutôt `fileReplace` avec plage.
    StringReplace { path: String, search: String, replace: String },
    /// Remplacement d’un segment (lignes, octets) — voir champs.
    FileReplace {
        path: String,
        range: ReplaceRangeV2,
        #[serde(rename = "newText")]
        new_text: String,
    },
    /// Fusion [RFC 7396](https://www.rfc-editor.org/rfc/rfc7396) (JSON *merge patch*) sur un
    /// document JSON racine objet, après validation de cohérence `schemaId` + `path`.
    ConfigJsonMergePatch {
        path: String,
        /// Portée logique (quel fichier d’espace on attend à ce `path` relatif).
        #[serde(rename = "schemaId")]
        schema_id: ConfigSchemaIdV2,
        /// Document *patch* (objet JSON) fusionné sur le fichier cible.
        patch: serde_json::Value,
    },
    /// Écrit le texte UTF-8 complet d’un chemin ressource (fichier créé ou tronqué remplacé).
    ResourceWriteText { path: String, content: String },
    /// Copie une ressource relatif-à-relatif dans le workspace.
    ResourceCopy {
        #[serde(rename = "fromPath")]
        from_path: String,
        #[serde(rename = "toPath")]
        to_path: String,
    },
}

/// Plage de remplacement ; clés internes = camelCase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplaceRangeV2 {
    /// Lignes **1-based** (début et fin **inclus**).
    #[serde(rename = "lines")]
    Lines {
        #[serde(rename = "startLine")]
        start_line: u32,
        #[serde(rename = "endLine")]
        end_line: u32,
    },
    /// Segments par indices octets UTF-8.
    #[serde(rename = "utf8Bytes")]
    Utf8Bytes {
        #[serde(rename = "startByte")]
        start_byte: usize,
        #[serde(rename = "endByte")]
        end_byte: usize,
    },
}

/// Remplacement ciblé fichier (alias logique de `FileReplace` pour noms d’outils “Copilot-like”).
pub type EditFileReplaceV2 = (String, ReplaceRangeV2, String);

impl EditProposalEnvelopeV2 {
    /// Règles sûres avant affichage diff (pas d’E/S disque ici).
    pub fn validate(&self) -> Result<(), EditProposalValidationError> {
        if self.version != 2 {
            return Err(EditProposalValidationError::Corrupt("version doit être 2"));
        }
        if self.id.is_empty() || self.id.len() > MAX_ID {
            return Err(EditProposalValidationError::ExceedsSizeLimit {
                field: "id",
                max: MAX_ID,
                got: self.id.len(),
            });
        }
        if self.summary.len() > MAX_SUMMARY {
            return Err(EditProposalValidationError::ExceedsSizeLimit {
                field: "summary",
                max: MAX_SUMMARY,
                got: self.summary.len(),
            });
        }
        if self.ops.is_empty() {
            return Err(EditProposalValidationError::Corrupt("ops ne peut pas être vide"));
        }
        if self.ops.len() > MAX_OPS {
            return Err(EditProposalValidationError::ExceedsSizeLimit {
                field: "ops.len",
                max: MAX_OPS,
                got: self.ops.len(),
            });
        }
        for op in &self.ops {
            op.validate()?;
        }
        Ok(())
    }
}

/// Préfixe sûr d’une ressource workspace (écriture / copie) — exclut sorties de bake et cache.
fn resource_workspace_prefix(path: &str) -> bool {
    const ALLOW: [&str; 5] = [
        "assets/",
        "shaders/",
        "src/",
        "editor/",
        "extensions/",
    ];
    ALLOW
        .iter()
        .any(|p| path.starts_with(p))
        && !path.starts_with("dist/")
        && !path.starts_with(".w3cache/")
}

impl EditOpV2 {
    /// Chemins lus/écrits (relatifs workspace) pour l’op ; `resourceCopy` retourne `[from, to]`.
    pub fn paths_touched(&self) -> Vec<&str> {
        match self {
            EditOpV2::StringReplace { path, .. }
            | EditOpV2::FileReplace { path, .. }
            | EditOpV2::ConfigJsonMergePatch { path, .. }
            | EditOpV2::ResourceWriteText { path, .. } => vec![path.as_str()],
            EditOpV2::ResourceCopy { from_path, to_path, .. } => {
                vec![from_path.as_str(), to_path.as_str()]
            }
        }
    }

    fn path_ok(path: &str) -> bool {
        if path.is_empty() || path.len() > MAX_PATH {
            return false;
        }
        if path.starts_with('/') || path.starts_with('\\') {
            return false;
        }
        let p = std::path::Path::new(path);
        p.components()
            .all(|c| !matches!(c, std::path::Component::ParentDir))
    }

    fn path_matches_config_schema(
        path: &str,
        schema_id: &ConfigSchemaIdV2,
    ) -> Result<(), EditProposalValidationError> {
        if !Self::path_ok(path) {
            return Err(EditProposalValidationError::BadPath);
        }
        let ok = match schema_id {
            ConfigSchemaIdV2::EditorShell => path == "editor/editor-ui.json",
            ConfigSchemaIdV2::Scene => path.starts_with("src/") && path.ends_with(".json"),
            ConfigSchemaIdV2::Assistant => path == "editor/assistant.json",
            ConfigSchemaIdV2::GenericJson => {
                resource_workspace_prefix(path) && path.ends_with(".json")
            }
        };
        if ok {
            Ok(())
        } else {
            Err(EditProposalValidationError::Corrupt(
                "schemaId incompatible avec le chemin cible",
            ))
        }
    }

    /// Valide l’opération.
    pub fn validate(&self) -> Result<(), EditProposalValidationError> {
        match self {
            EditOpV2::StringReplace {
                path,
                search,
                replace,
            } => {
                if !Self::path_ok(path) {
                    return Err(EditProposalValidationError::BadPath);
                }
                if search.is_empty() {
                    return Err(EditProposalValidationError::Corrupt("stringReplace: search vide interdit"));
                }
                for (f, t) in [("search", search.as_str()), ("replace", replace.as_str())] {
                    if t.len() > MAX_REPLACEMENT {
                        return Err(EditProposalValidationError::ExceedsSizeLimit {
                            field: f,
                            max: MAX_REPLACEMENT,
                            got: t.len(),
                        });
                    }
                }
            }
            EditOpV2::FileReplace { path, range, new_text } => {
                if !Self::path_ok(path) {
                    return Err(EditProposalValidationError::BadPath);
                }
                if new_text.len() > MAX_REPLACEMENT {
                    return Err(EditProposalValidationError::ExceedsSizeLimit {
                        field: "newText",
                        max: MAX_REPLACEMENT,
                        got: new_text.len(),
                    });
                }
                if let ReplaceRangeV2::Lines { start_line, end_line } = range {
                    if *start_line == 0
                        || *end_line == 0
                        || *end_line < *start_line
                        || *end_line - *start_line > 1_000_000
                    {
                        return Err(EditProposalValidationError::Corrupt("lignes invalides"));
                    }
                }
                if let ReplaceRangeV2::Utf8Bytes { start_byte, end_byte } = range {
                    if *end_byte < *start_byte {
                        return Err(EditProposalValidationError::Corrupt("octets invalides"));
                    }
                }
            }
            EditOpV2::ConfigJsonMergePatch {
                path,
                schema_id,
                patch,
            } => {
                Self::path_matches_config_schema(path, schema_id)?;
                if !patch.is_object() {
                    return Err(EditProposalValidationError::Corrupt("patch attend un objet JSON"));
                }
                let n = match serde_json::to_string(patch) {
                    Ok(s) => s.len(),
                    Err(_) => return Err(EditProposalValidationError::Corrupt("patch non sérialisable")),
                };
                if n > MAX_JSON_PATCH_VALUE {
                    return Err(EditProposalValidationError::ExceedsSizeLimit {
                        field: "patch",
                        max: MAX_JSON_PATCH_VALUE,
                        got: n,
                    });
                }
            }
            EditOpV2::ResourceWriteText { path, content } => {
                if !Self::path_ok(path) {
                    return Err(EditProposalValidationError::BadPath);
                }
                if !resource_workspace_prefix(path) {
                    return Err(EditProposalValidationError::BadPath);
                }
                if content.len() > MAX_REPLACEMENT {
                    return Err(EditProposalValidationError::ExceedsSizeLimit {
                        field: "content",
                        max: MAX_REPLACEMENT,
                        got: content.len(),
                    });
                }
            }
            EditOpV2::ResourceCopy {
                from_path,
                to_path,
            } => {
                for p in [from_path.as_str(), to_path.as_str()] {
                    if !Self::path_ok(p) {
                        return Err(EditProposalValidationError::BadPath);
                    }
                    if !resource_workspace_prefix(p) {
                        return Err(EditProposalValidationError::BadPath);
                    }
                }
                if from_path == to_path {
                    return Err(EditProposalValidationError::Corrupt(
                        "resourceCopy: chemins from/to identiques",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_roundtrip() {
        let s = std::include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/phases/phase-k/assistant-edit-proposal-v2.example.json"
        ));
        let p: EditProposalEnvelopeV2 = serde_json::from_str(s).expect("parse");
        p.validate().expect("validates");
    }

    #[test]
    fn typed_ops_example_validates() {
        let s = std::include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/phases/phase-k/assistant-edit-proposal-v2-typed-ops.example.json"
        ));
        let p: EditProposalEnvelopeV2 = serde_json::from_str(s).expect("parse");
        p.validate().expect("validates");
    }

    #[test]
    fn reject_parent_in_path() {
        let p = EditProposalEnvelopeV2 {
            version: 2,
            id: "1".to_string(),
            summary: "x".to_string(),
            ops: vec![EditOpV2::StringReplace {
                path: "../evil".to_string(),
                search: "a".to_string(),
                replace: "b".to_string(),
            }],
        };
        assert_eq!(p.validate().unwrap_err(), EditProposalValidationError::BadPath);
    }

    #[test]
    fn config_schema_mismatch() {
        let p = EditProposalEnvelopeV2 {
            version: 2,
            id: "1".to_string(),
            summary: "x".to_string(),
            ops: vec![EditOpV2::ConfigJsonMergePatch {
                path: "editor/wrong.json".to_string(),
                schema_id: ConfigSchemaIdV2::EditorShell,
                patch: serde_json::json!({}),
            }],
        };
        assert_eq!(
            p.validate().unwrap_err(),
            EditProposalValidationError::Corrupt("schemaId incompatible avec le chemin cible",)
        );
    }

    #[test]
    fn config_merge_patch_size_ok() {
        let p = EditProposalEnvelopeV2 {
            version: 2,
            id: "1".to_string(),
            summary: "x".to_string(),
            ops: vec![EditOpV2::ConfigJsonMergePatch {
                path: "src/default.scene.json".to_string(),
                schema_id: ConfigSchemaIdV2::Scene,
                patch: serde_json::json!({ "version": 2 }),
            }],
        };
        p.validate().expect("valides");
    }

    #[test]
    fn valid_replace_line() {
        let p = EditProposalEnvelopeV2 {
            version: 2,
            id: "id-1".to_string(),
            summary: "s".to_string(),
            ops: vec![EditOpV2::FileReplace {
                path: "shaders/pbr.wgsl".to_string(),
                range: ReplaceRangeV2::Lines {
                    start_line: 10,
                    end_line: 10,
                },
                new_text: "fn f() {}".to_string(),
            }],
        };
        p.validate().expect("ok");
    }
}
