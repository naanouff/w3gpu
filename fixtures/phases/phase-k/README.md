# phase-k — workspace éditeur (jalon)

Fixtures pour **Phase K** (workspace, extensions) et le ticket **PHASE-B-EDITOR-UI** (shell, `editor-ui.json` — **fidélité** [v3-hifi.css](../../../docs/design/v3-hifi.css) : voir [phase-B-editor-ui-ux-implementation.md](../../../docs/tickets/phase-B-editor-ui-ux-implementation.md)).

## Contenu

| Chemin | Rôle |
|--------|------|
| [`editor-ui.json`](editor-ui.json) | Thème, stage, **8 modes** (rail) — consommé par le shell Vite + `w3d-editor` natif. |
| [`workspace/`](workspace/) | Racine **projet** témoin (tranche [Goals.md](../../../docs/Goals.md) : `assets/`, `src/`, `shaders/`, `dist/`, `.w3cache/`). |
| [`extensions/hello_stub/`](extensions/hello_stub/) | Extension tierce **stub** (`plugin.json` + README) — chargement réel = implémentation future. |
| [`expected.md`](expected.md) | Checklist cible pour bake **`.w3db`** + preuve d’extension (DOD). |
| `assistant.json`, `assistant-*.json` | Assistant (optionnel) — voir [editor/README.md](../../editor/README.md). |

## Reproduction (natif / web)

**Natif** : `cargo xtask editor` ou `cargo run -p w3d-editor` — lit `editor-ui.json` par défaut. Pour pointer un **workspace** de dev : option future `--workspace` (ou ouverture via UI) — le dossier témoin est `workspace/` ici.

**Web** : `cd www && npm install && npm run build:wasm && npm run dev` — le shell importe le même `editor-ui.json` (build Vite).

## Tests

- `cargo test -p w3d-editor` : module `phase_k_workspace` (arborescence `phase-k/` sur disque).
- `www` : tests Vitest sur `editor-ui.json` (8 modes, rail).

## Suite (hors DOD de ce seul jalon)

- Ouvrir **`workspace/`** dans l’éditeur, brancher l’**outliner** sur l’**arbre ECS** + **sélection** outliner / viewport (voir [Phase K — ticket](../../../docs/tickets/phase-K-editeur-workspaces.md)).
