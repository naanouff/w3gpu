# Fixture `phase-b` — graphe de rendu déclaratif

Projet de test pour [Phase B — Graphe de rendu & compute](../../docs/tickets/phase-B-graphe-rendu-compute.md).

## Contenu

| Fichier | Rôle |
|---------|------|
| [`render_graph.json`](render_graph.json) | Document v0 : ressources + passes (compute puis raster vers `hdr_color` + depth `scene_depth`) — schéma [`docs/schemas/render-graph-v0.md`](../../docs/schemas/render-graph-v0.md). |
| [`shaders/`](shaders/) | WGSL référencés par le JSON (exécutés par `run_graph_v0_checksum` côté natif). |
| [`expected.md`](expected.md) | Ordre des passes et lien vers tests. |

## Prérequis

- Rust stable.
- Le jalon v0 inclut **parse + validation** (`w3drs-render-graph`) et **exécuteur checksum natif** (`w3drs-renderer`).

## Commandes

```bash
cargo test -p w3drs-render-graph
cargo test -p w3drs-renderer --test phase_b_graph_exec
cargo xtask check
```

**Exemple CLI (checksum une frame, même pipeline que l’intégration test)** — depuis la racine `w3drs/` :

```bash
cargo run -p phase-b-graph --release
```

Aucune fenêtre : c’est un **binaire headless** (message sur la console puis fin, ~1 s). Un **GPU** et des pilotes à jour sont requis (sinon message d’erreur explicite).

(Ce dossier, **ou** un chemin absolu/réel passé en argument — pas un placeholder ; voir [`../../examples/phase-b-graph/src/main.rs`](../../examples/phase-b-graph/src/main.rs).)

Le test `phase_b_graph_exec` exécute le graphe sur **GPU natif** (`run_graph_v0_checksum`) et vérifie que **deux** soumissions consécutives produisent le même hachage readback sur `hdr_color`.

**Web** : copie miroir sous [`www/public/phase-b/`](../../www/public/phase-b/) (servie par Vite) ; `www/src/main.ts` appelle **`w3drsValidateRenderGraphV0`** au boot (console `[w3drs] Phase B render graph: validate OK…`). Après changement Rust du bindgen : `npm run build:wasm`.

## Suite

- Généraliser l’exécuteur (bindings, barrières, resize) ; brancher le **même** JSON côté WASM.
- Même graphe chargé côté **WASM** une fois l’API stable.
