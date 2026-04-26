//! Exécution d’`EditProposalEnvelopeV2` sur le disque d’un **workspace** : transaction par
//! sauvegarde initiale, rollback sur échec, *undo* optionnel d’un apply réussi.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use w3drs_assistant_api::apply_one_to_utf8;
use w3drs_assistant_api::EditApplyError;
use w3drs_assistant_api::EditOpV2;
use w3drs_assistant_api::EditProposalEnvelopeV2;
use w3drs_assistant_api::EditProposalValidationError;

/// Erreur côté hôte (validation + I/O + application pure).
#[derive(thiserror::Error, Debug)]
pub enum WorkspaceApplyError {
    /// Enveloppe refusée par le validateur (même texte qu’`EditProposalValidationError`).
    #[error(transparent)]
    Envelope(#[from] EditProposalValidationError),
    #[error("apply op: {0}")]
    Edit(#[from] EditApplyError),
    #[error("E/S: {0}")]
    Io(#[from] io::Error),
    #[error("resourceCopy: {0}")]
    Copy(&'static str),
}

/// Rapport d’un apply : chemins (relatifs) écrits, snapshot pour *undo* optionnel.
#[derive(Debug)]
pub struct ApplyRunReport {
    pub rel_paths_finalized: Vec<String>,
    pub snapshot: Option<ApplyFilesSnapshot>,
}

/// Binaire par fichier **avant** l’`apply` (fichier absent: `None`).
#[derive(Debug)]
pub struct ApplyFilesSnapshot {
    pub root: PathBuf,
    rel_before: Vec<(String, Option<Vec<u8>>)>,
}

impl ApplyFilesSnapshot {
    /// Revenir à l’état d’**origine** (désactive les modifications; suppression si l’existant
    /// n’existait pas auparavant).
    pub fn undo(self) -> io::Result<()> {
        for (rel, was) in self.rel_before.into_iter().rev() {
            let p = self.root.join(&rel);
            if let Some(bytes) = was {
                if let Some(parent) = p.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(p, &bytes)?;
            } else {
                let _ = std::fs::remove_file(&p);
            }
        }
        Ok(())
    }
}

/// Chemins (relatifs) pouvant subir I/O: cibles toutes, plus les sources d’une copie.
fn all_touched_rels(proposal: &EditProposalEnvelopeV2) -> BTreeSet<String> {
    let mut s = BTreeSet::new();
    for o in &proposal.ops {
        for p in o.paths_touched() {
            s.insert(p.to_string());
        }
    }
    s
}

/// Lit UTF-8 depuis `work` ou le disque.
fn current_utf8(
    root: &Path,
    work: &BTreeMap<String, String>,
    rel: &str,
) -> io::Result<Option<String>> {
    if let Some(x) = work.get(rel) {
        return Ok(Some(x.clone()));
    }
    let p = root.join(rel);
    if p.is_file() {
        return Ok(Some(std::fs::read_to_string(p)?));
    }
    Ok(None)
}

/// Lit octets: priorité *work* (encodage UTF-8) sinon fichier.
fn read_source_bytes(
    root: &Path,
    work: &BTreeMap<String, String>,
    from: &str,
) -> io::Result<Vec<u8>> {
    if let Some(s) = work.get(from) {
        return Ok(s.as_bytes().to_vec());
    }
    let p = root.join(from);
    if p.is_file() {
        return std::fs::read(p);
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("source resourceCopy: {from}"),
    ))
}

/// Point d’entrée unique: **toute** l’E/S d’un *Apply* (produit / assistant).
pub fn apply_proposal_v2(
    workspace_root: &Path,
    proposal: &EditProposalEnvelopeV2,
) -> Result<ApplyRunReport, WorkspaceApplyError> {
    proposal.validate()?;
    for o in &proposal.ops {
        o.validate()?;
    }
    let touched = all_touched_rels(proposal);
    let mut rel_before: Vec<(String, Option<Vec<u8>>)> = Vec::new();
    for rel in &touched {
        let p = workspace_root.join(rel);
        if p.is_file() {
            rel_before.push((rel.clone(), Some(std::fs::read(&p)?)));
        } else {
            rel_before.push((rel.clone(), None));
        }
    }
    let restore_from_snapshot = |rel_b: &[(String, Option<Vec<u8>>)]| -> io::Result<()> {
        for (rel, was) in rel_b.iter() {
            let p = workspace_root.join(rel);
            if let Some(bytes) = was {
                if let Some(parent) = p.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&p, bytes)?;
            } else {
                let _ = std::fs::remove_file(&p);
            }
        }
        Ok(())
    };
    let mut work: BTreeMap<String, String> = BTreeMap::new();
    let run: Result<(), WorkspaceApplyError> = (|| {
        for op in &proposal.ops {
            if let EditOpV2::ResourceCopy { from_path, to_path } = op {
                work.remove(to_path);
                let bytes = read_source_bytes(workspace_root, &work, from_path)?;
                let to_abs = workspace_root.join(to_path);
                if let Some(parent) = to_abs.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&to_abs, &bytes)?;
                continue;
            }
            let path = match op {
                EditOpV2::StringReplace { path, .. }
                | EditOpV2::FileReplace { path, .. }
                | EditOpV2::ConfigJsonMergePatch { path, .. }
                | EditOpV2::ResourceWriteText { path, .. } => path.as_str(),
                EditOpV2::ResourceCopy { .. } => unreachable!(),
            };
            let current = current_utf8(workspace_root, &work, path)?;
            let next = apply_one_to_utf8(op, path, current)?;
            work.insert(path.to_string(), next);
        }
        for (rel, s) in &work {
            let p = workspace_root.join(rel);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(p, s.as_bytes())?;
        }
        Ok(())
    })();
    match run {
        Ok(()) => {
            let snap = Some(ApplyFilesSnapshot {
                root: workspace_root.to_path_buf(),
                rel_before: rel_before
                    .into_iter()
                    .map(|(a, b)| (a, b))
                    .collect(),
            });
            let mut rfp = BTreeSet::new();
            for o in &proposal.ops {
                match o {
                    EditOpV2::StringReplace { path, .. }
                    | EditOpV2::FileReplace { path, .. }
                    | EditOpV2::ConfigJsonMergePatch { path, .. }
                    | EditOpV2::ResourceWriteText { path, .. } => {
                        rfp.insert(path.clone());
                    }
                    EditOpV2::ResourceCopy { to_path, .. } => {
                        rfp.insert(to_path.clone());
                    }
                }
            }
            let mut v: Vec<String> = rfp.into_iter().collect();
            v.sort();
            Ok(ApplyRunReport {
                rel_paths_finalized: v,
                snapshot: snap,
            })
        }
        Err(e) => {
            restore_from_snapshot(&rel_before)?;
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phase_k_ws() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("fixtures")
            .join("phases")
            .join("phase-k")
            .join("workspace")
    }

    #[test]
    fn apply_proposal_succeeds_on_empty_extra_file_and_undo() {
        let d = std::env::temp_dir().join(format!(
            "w3d-apply-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&d).expect("d");
        let src = phase_k_ws();
        for p in &["shaders", "src"] {
            let _ = d.join(p);
            w3d_copy_dir_all(&src.join(p), &d.join(p));
        }
        // fichier texte témoin relatif, absent au départ
        let p = d.join("src/assistant-test.txt");
        if p.exists() {
            std::fs::remove_file(&p).ok();
        }
        let op = [
            EditOpV2::ResourceWriteText {
                path: "src/assistant-test.txt".to_string(),
                content: "A".to_string(),
            },
            EditOpV2::StringReplace {
                path: "src/assistant-test.txt".to_string(),
                search: "A".to_string(),
                replace: "B".to_string(),
            },
        ];
        let env = EditProposalEnvelopeV2 {
            version: 2,
            id: "t-apply-1".to_string(),
            summary: "test".to_string(),
            ops: op.to_vec(),
        };
        let r = apply_proposal_v2(&d, &env).expect("apply");
        let txt = std::fs::read_to_string(d.join("src/assistant-test.txt")).expect("read");
        assert_eq!(txt, "B");
        let snap = r.snapshot.expect("snap");
        snap.undo().expect("undo");
        assert!(!d.join("src/assistant-test.txt").is_file() || {
            // si existait, serait revenu: pas de fichier
            !d.join("src/assistant-test.txt").exists()
        });
        // avant apply le fichier n’existait pas → après undo, absent
        assert!(!d.join("src/assistant-test.txt").is_file());
    }

    /// Copie *min* récursive
    fn w3d_copy_dir_all(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).ok();
        for e in std::fs::read_dir(from).expect("r") {
            let e = e.expect("e");
            let t = to.join(e.file_name());
            if e.path().is_dir() {
                w3d_copy_dir_all(&e.path(), &t);
            } else {
                std::fs::create_dir_all(to).ok();
                std::fs::copy(&e.path(), &t).expect("cp");
            }
        }
    }

    #[test]
    fn failed_apply_rolls_back_default_scene() {
        let d = std::env::temp_dir().join(format!(
            "w3d-apply-rollback-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        w3d_copy_dir_all(&phase_k_ws().join("src"), &d.join("src"));
        let before = std::fs::read_to_string(d.join("src/default.scene.json")).expect("pre");
        let dead = vec![EditOpV2::StringReplace {
            path: "src/nosuch.txt".to_string(),
            search: "x".to_string(),
            replace: "y".to_string(),
        }];
        assert!(dead[0].validate().is_ok());
        let env = EditProposalEnvelopeV2 {
            version: 2,
            id: "c".to_string(),
            summary: "c".to_string(),
            ops: dead,
        };
        assert!(env.validate().is_ok());
        let err = apply_proposal_v2(&d, &env);
        assert!(err.is_err());
        let after = std::fs::read_to_string(d.join("src/default.scene.json")).expect("post");
        assert_eq!(before, after, "aucune modif en cas d’échec");
    }
}