# phase-k — workspace éditeur (jalon)

Fixture pour **Phase K** (workspace, extensions) et le ticket **PHASE-B-EDITOR-UI** (shell `www/`, thème + layout data-driven).

## Fichiers

| Fichier | Rôle |
|---------|------|
| [`editor-ui.json`](editor-ui.json) | Thème d’enveloppe (`appearance`) + métadonnées de stage + liste des **8 modes** (rail). Consommé par l’appli Vite en import direct (pas de logique d’écran en dur hors defaults). |

## Reproduction (natif / web)

**Natif** : `cargo xtask editor` ou `cargo run -p w3d-editor` — lit ce fichier (depuis la racine du dépôt par défaut). Voir [`editor/README.md`](../../editor/README.md).

1. `cd www && npm install && npm run build:wasm && npm run vite` (WASM) : le shell importe le même `editor-ui.json` côté build.
2. Ouvrir l’URL indiquée par Vite.

## Tests

Les tests `www` résolvent `fixtures/phases/phase-k/editor-ui.json` par chemin relatif au dépôt pour vérifier la spec (8 modes, ids stables).
