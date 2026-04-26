//! Logique d’**application** des `EditOpV2` (sans I/O) : fusions JSON RFC 7396, remplacements
//! texte, écriture de ressource. L’E/S, les transactions clés et `resourceCopy` : [`w3d_editor`].
//!
//! Enchaînement: sur couche d’envelopre déjà validé (`EditProposalEnvelopeV2::validate`).

use serde_json::Value;

use crate::edit_proposal_v2::{EditOpV2, ReplaceRangeV2};

/// Erreur d’**application** sur le contenu (fichier attendu, JSON, plage).
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum EditApplyError {
    #[error("fichier manquant pour cette op: {0}")]
    MissingFile(String),
    #[error("stringReplace: clé de recherche vide: {0}")]
    StringReplaceEmptySearch(String),
    #[error("stringReplace: aucun match: {0} (search)")]
    StringReplaceNoMatch(String, String),
    #[error("fileReplace: plage hors fichier (lignes): path {0}, n={1}, {2}–{3}")]
    FileReplaceLinesOob(String, usize, u32, u32),
    #[error("fileReplace: plage octets absurde: path {0} len {1} [{2},{3})")]
    FileReplaceBytesOob(String, usize, usize, usize),
    #[error("configJson: parse base: {0}")]
    ConfigJsonBaseParse(String, String),
    #[error("configJson: cible n’est pas un objet racine: {0}")]
    ConfigJsonNotObject(String),
    #[error("configJson: sérialisation: {0}")]
    ConfigJsonSer(String),
    #[error("op pure inapplicable (p.ex. resourceCopy): {0}")]
    UnsupportedInPureContext(&'static str),
}

/// RFC 7396 (*JSON merge patch*), cible = objet, patch = objet.
pub fn merge_json_rfc7396_in_place(target: &mut Value, patch: &Value) {
    if !patch.is_object() {
        *target = patch.clone();
        return;
    }
    let Some(t_obj) = target.as_object_mut() else {
        *target = patch.clone();
        return;
    };
    for (k, v) in patch.as_object().unwrap() {
        if v.is_null() {
            t_obj.remove(k);
        } else if let Some(existing) = t_obj.get_mut(k) {
            if existing.is_object() && v.is_object() {
                merge_json_rfc7396_in_place(existing, v);
            } else {
                t_obj.insert(k.clone(), v.clone());
            }
        } else {
            t_obj.insert(k.clone(), v.clone());
        }
    }
}

/// Applique **une** op sachant le contenu UTF-8 courant (ou `None` si le fichier n’existe pas).
/// `path` = même chaîne relatif qu’`op` (contrôle d’intégrité).
pub fn apply_one_to_utf8(
    op: &EditOpV2,
    op_path: &str,
    current: Option<String>,
) -> Result<String, EditApplyError> {
    match op {
        EditOpV2::StringReplace { path, search, replace } => {
            if path != op_path {
                return Err(EditApplyError::UnsupportedInPureContext("path ne correspond pas à l’op"));
            }
            if search.is_empty() {
                return Err(EditApplyError::StringReplaceEmptySearch(path.to_string()));
            }
            let s = current
                .ok_or_else(|| EditApplyError::MissingFile(path.to_string()))?
                .replace('\r', "");
            if !s.contains(search) {
                return Err(EditApplyError::StringReplaceNoMatch(
                    path.to_string(),
                    search.clone(),
                ));
            }
            Ok(s.replace(search, replace))
        }
        EditOpV2::FileReplace {
            path, range, new_text, ..
        } => {
            if path != op_path {
                return Err(EditApplyError::UnsupportedInPureContext("path ne correspond pas à l’op"));
            }
            let s = current
                .ok_or_else(|| EditApplyError::MissingFile(path.to_string()))?
                .replace('\r', "");
            apply_file_replace_in_str(&s, path, range, new_text)
        }
        EditOpV2::ConfigJsonMergePatch { path, patch, .. } => {
            if path != op_path {
                return Err(EditApplyError::UnsupportedInPureContext("path ne correspond pas à l’op"));
            }
            let s = current
                .ok_or_else(|| EditApplyError::MissingFile(path.to_string()))?;
            let mut base: Value = serde_json::from_str(s.as_str()).map_err(|e| {
                EditApplyError::ConfigJsonBaseParse(path.to_string(), e.to_string())
            })?;
            if !base.is_object() {
                return Err(EditApplyError::ConfigJsonNotObject(path.to_string()));
            }
            if !patch.is_object() {
                // déjà rejeté par `validate` ; défense
                return Err(EditApplyError::ConfigJsonNotObject(path.to_string()));
            }
            merge_json_rfc7396_in_place(&mut base, patch);
            serde_json::to_string_pretty(&base).map_err(|e| EditApplyError::ConfigJsonSer(e.to_string()))
        }
        EditOpV2::ResourceWriteText { path, content } => {
            if path != op_path {
                return Err(EditApplyError::UnsupportedInPureContext("path ne correspond pas à l’op"));
            }
            Ok(content.to_string())
        }
        EditOpV2::ResourceCopy { .. } => Err(EditApplyError::UnsupportedInPureContext(
            "utiliser l’E/S workspace pour resourceCopy",
        )),
    }
}

fn apply_file_replace_in_str(
    s: &str,
    path: &str,
    range: &ReplaceRangeV2,
    new_text: &str,
) -> Result<String, EditApplyError> {
    match range {
        ReplaceRangeV2::Lines {
            start_line,
            end_line,
        } => {
            let lines: Vec<&str> = s.split('\n').collect();
            let n = lines.len();
            let sli = *start_line as usize;
            let eli = *end_line as usize;
            if sli < 1 || eli < sli || sli > n || eli > n {
                return Err(EditApplyError::FileReplaceLinesOob(
                    path.to_string(),
                    n,
                    *start_line,
                    *end_line,
                ));
            }
            let pre = if sli > 1 { lines[0..sli - 1].join("\n") } else { String::new() };
            let after = if eli < n { lines[eli..].join("\n") } else { String::new() };
            if pre.is_empty() {
                if after.is_empty() {
                    return Ok(new_text.to_string());
                }
                return Ok(format!("{new_text}\n{after}"));
            }
            if after.is_empty() {
                return Ok(format!("{pre}\n{new_text}"));
            }
            Ok(format!("{pre}\n{new_text}\n{after}"))
        }
        ReplaceRangeV2::Utf8Bytes { start_byte, end_byte } => {
            if *end_byte < *start_byte || *start_byte > s.len() || *end_byte > s.len() {
                return Err(EditApplyError::FileReplaceBytesOob(
                    path.to_string(),
                    s.len(),
                    *start_byte,
                    *end_byte,
                ));
            }
            let mut out = String::new();
            out.push_str(&s[..*start_byte]);
            out.push_str(new_text);
            out.push_str(&s[*end_byte..]);
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConfigSchemaIdV2;
    use serde_json::json;

    #[test]
    fn merge_merges_nested() {
        let mut t = json!({ "a": 1, "b": { "c": 2 } });
        let p = json!({ "b": { "c": 3, "d": 4 } });
        merge_json_rfc7396_in_place(&mut t, &p);
        assert_eq!(t["a"], 1);
        assert_eq!(t["b"]["c"], 3);
        assert_eq!(t["b"]["d"], 4);
    }

    #[test]
    fn string_replace_all() {
        let op = EditOpV2::StringReplace {
            path: "x.wgsl".to_string(),
            search: "a".to_string(),
            replace: "b".to_string(),
        };
        let s = apply_one_to_utf8(&op, "x.wgsl", Some("a a".to_string())).expect("x");
        assert_eq!(s, "b b");
    }

    #[test]
    fn file_replace_line_block() {
        let op = EditOpV2::FileReplace {
            path: "f".to_string(),
            range: ReplaceRangeV2::Lines {
                start_line: 2,
                end_line: 2,
            },
            new_text: "Z".to_string(),
        };
        let s = apply_one_to_utf8(
            &op,
            "f",
            Some("A\nB\nC".to_string()),
        )
        .expect("ok");
        assert_eq!(s, "A\nZ\nC");
    }

    #[test]
    fn config_merge() {
        let op = EditOpV2::ConfigJsonMergePatch {
            path: "x.json".to_string(),
            schema_id: ConfigSchemaIdV2::GenericJson,
            patch: json!({ "label": "L2" }),
        };
        let out = apply_one_to_utf8(
            &op,
            "x.json",
            Some(r#"{ "version": 1, "id": "x", "label": "L" }"#.to_string()),
        )
        .expect("ok");
        let v: Value = serde_json::from_str(&out).expect("json");
        assert_eq!(v["label"], "L2");
    }
}
