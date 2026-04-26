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

/// Erreur de **validation côté éditeur** (hors sémantique de fichiers : pas d’E/S ici).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditProposalValidationError {
    /// Enveloppe mal formée.
    Corrupt(&'static str),
    /// Chemin interdit (`..`, absolu, vide).
    BadPath,
    /// Texte d’opération dépassant un plafond sûr (anti bomb).
    ExceedsSizeLimit { field: &'static str, max: usize, got: usize },
}

const MAX_PATH: usize = 1_024;
const MAX_REPLACEMENT: usize = 1_000_000;
const MAX_ID: usize = 64;
const MAX_SUMMARY: usize = 8_192;
const MAX_OPS: usize = 1_000;

/// Enveloppe versionnée; `version` doit être 2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditProposalEnvelopeV2 {
    pub version: u8,
    /// Identifiant d’idempotence / suivi d’UI (p.ex. UUID court).
    pub id: String,
    pub summary: String,
    pub ops: Vec<EditOpV2>,
}

/// Opération ciblée, extensible par `op` tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl EditOpV2 {
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
