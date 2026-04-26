//! Parse / validation de `fixtures/phases/phase-k/editor-ui.json` (aligné sur `www`).

use std::path::Path;

use serde::Deserialize;
use thiserror::Error;

/// Identifiants de modes, ordre figé (Build → … → Ship).
pub const EDITOR_MODE_IDS: [&str; 8] = [
    "build", "paint", "sculpt", "logic", "animate", "light", "play", "ship",
];

/// Document typé, équivalent `parseEditorUiV1` côté `www/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorUi {
    pub version: u8,
    pub shell: Shell,
    pub stage: Stage,
    pub modes: [ModeEntry; 8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    Light,
    Dark,
}

impl Appearance {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

impl std::fmt::Display for Appearance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shell {
    pub appearance: Appearance,
    pub layout: Layout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    /// Largeur logique du rail (px CSS / points UI) — mêmes valeurs que le JSON.
    pub rail_width_css_px: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage {
    pub title: String,
    pub default_breadcrumb: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeEntry {
    pub id: String,
    pub label: String,
    pub key_hint: String,
}

#[derive(Debug, Deserialize)]
struct FileDocument {
    version: u8,
    shell: FileShell,
    stage: FileStage,
    modes: Vec<FileMode>,
}

#[derive(Debug, Deserialize)]
struct FileShell {
    appearance: String,
    layout: FileLayout,
}

#[derive(Debug)]
struct FileLayout {
    rail_width_css_px: u32,
}

#[derive(Debug, Deserialize)]
struct FileStage {
    title: String,
    #[serde(rename = "defaultBreadcrumb")]
    default_breadcrumb: String,
}

#[derive(Debug)]
struct FileMode {
    id: String,
    label: String,
    key_hint: String,
}

// serde: JSON utilise camelCase
impl<'de> Deserialize<'de> for FileMode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct M {
            id: String,
            label: String,
            #[serde(rename = "keyHint")]
            key_hint: String,
        }
        let m = M::deserialize(deserializer)?;
        Ok(FileMode {
            id: m.id,
            label: m.label,
            key_hint: m.key_hint,
        })
    }
}

// railWidthCssPx en camelCase
impl<'de> Deserialize<'de> for FileLayout {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct L {
            #[serde(rename = "railWidthCssPx")]
            rail_width_css_px: u32,
        }
        let l = L::deserialize(deserializer)?;
        Ok(FileLayout {
            rail_width_css_px: l.rail_width_css_px,
        })
    }
}

#[derive(Debug, Error)]
pub enum EditorUiError {
    #[error("editor-ui: JSON: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("editor-ui: {0}")]
    Invalid(String),
}

/// Lit un fichier (UTF-8) et valide.
pub fn load_editor_config_from_path(path: &Path) -> Result<EditorUi, EditorUiError> {
    let s = std::fs::read_to_string(path)
        .map_err(|e| EditorUiError::Invalid(format!("lecture {}: {e}", path.display())))?;
    parse_editor_config_str(&s)
}

/// Parse + validation (mêmes règles que `www/src/editor/editorConfig.ts` — version, rail, 8 modes ordonnés).
pub fn parse_editor_config_str(s: &str) -> Result<EditorUi, EditorUiError> {
    let f: FileDocument = serde_json::from_str(s)?;
    validate_file(&f)
}

fn validate_file(f: &FileDocument) -> Result<EditorUi, EditorUiError> {
    if f.version != 1 {
        return Err(EditorUiError::Invalid(format!("version 1 requise, reçu {}", f.version)));
    }
    let appearance = match f.shell.appearance.as_str() {
        "light" => Appearance::Light,
        "dark" => Appearance::Dark,
        a => {
            return Err(EditorUiError::Invalid(format!("shell.appearance inattendu: {a}")));
        }
    };
    let w = f.shell.layout.rail_width_css_px;
    if !(32..=200).contains(&w) {
        return Err(EditorUiError::Invalid(format!(
            "shell.layout.railWidthCssPx hors plage 32..=200: {w}"
        )));
    }
    if f.stage.title.is_empty() {
        return Err(EditorUiError::Invalid("stage.title vide".into()));
    }
    if f.modes.len() != 8 {
        return Err(EditorUiError::Invalid(format!("exactement 8 modes, reçu {}", f.modes.len())));
    }
    let mut modes: [ModeEntry; 8] = std::array::from_fn(|_| {
        ModeEntry {
            id: String::new(),
            label: String::new(),
            key_hint: String::new(),
        }
    });
    for (i, m) in f.modes.iter().enumerate() {
        if m.id != EDITOR_MODE_IDS[i] {
            return Err(EditorUiError::Invalid(format!(
                "mode[{}] id: attendu « {} », reçu « {} »",
                i,
                EDITOR_MODE_IDS[i],
                m.id
            )));
        }
        if m.label.is_empty() {
            return Err(EditorUiError::Invalid(format!("label vide (mode {i})")));
        }
        modes[i] = ModeEntry {
            id: m.id.clone(),
            label: m.label.clone(),
            key_hint: m.key_hint.clone(),
        };
    }
    Ok(EditorUi {
        version: 1,
        shell: Shell {
            appearance,
            layout: Layout {
                rail_width_css_px: w,
            },
        },
        stage: Stage {
            title: f.stage.title.clone(),
            default_breadcrumb: f.stage.default_breadcrumb.clone(),
        },
        modes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn path_fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("fixtures")
            .join("phases")
            .join("phase-k")
            .join("editor-ui.json")
    }

    #[test]
    fn fixture_ok() {
        let p = path_fixture();
        let d = load_editor_config_from_path(&p).expect("parse fixture");
        assert_eq!(d.shell.appearance, Appearance::Dark);
        assert_eq!(d.shell.layout.rail_width_css_px, 48);
        assert_eq!(d.modes[0].id, "build");
        assert_eq!(d.modes[7].id, "ship");
    }

    #[test]
    fn err_version() {
        let base = std::fs::read_to_string(path_fixture()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&base).unwrap();
        let mut o = v.as_object().unwrap().clone();
        o.insert("version".to_string(), serde_json::json!(2));
        let bad = serde_json::to_string(&o).unwrap();
        assert!(parse_editor_config_str(&bad).is_err());
    }

    #[test]
    fn err_appearance_rail_title_modes() {
        let base = std::fs::read_to_string(path_fixture()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&base).unwrap();
        let mut o = v.as_object().unwrap().clone();
        o.insert(
            "shell".to_string(),
            serde_json::json!({ "appearance": "grey", "layout": { "railWidthCssPx": 72 } }),
        );
        assert!(parse_editor_config_str(&serde_json::to_string(&o).unwrap()).is_err());
        o.insert(
            "shell".to_string(),
            serde_json::json!({ "appearance": "dark", "layout": { "railWidthCssPx": 10 } }),
        );
        assert!(parse_editor_config_str(&serde_json::to_string(&o).unwrap()).is_err());
        o.insert("shell".to_string(), v["shell"].clone());
        let st = o.get_mut("stage").unwrap().as_object_mut().unwrap();
        st.insert("title".to_string(), serde_json::json!(""));
        assert!(parse_editor_config_str(&serde_json::to_string(&o).unwrap()).is_err());
        o.insert("stage".to_string(), v["stage"].clone());
        o.insert("modes".to_string(), serde_json::json!([]));
        assert!(parse_editor_config_str(&serde_json::to_string(&o).unwrap()).is_err());
        o.insert("modes".to_string(), v["modes"].clone());
        let m = o.get_mut("modes").unwrap().as_array_mut().unwrap();
        m[0] = serde_json::json!({ "id": "paint", "label": "x", "keyHint": "B" });
        assert!(parse_editor_config_str(&serde_json::to_string(&o).unwrap()).is_err());
    }
}
