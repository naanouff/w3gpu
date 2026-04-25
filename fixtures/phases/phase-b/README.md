# Fixture `phase-b` — graphe de rendu déclaratif

Projet de test pour [Phase B — Graphe de rendu & compute](../../docs/tickets/phase-B-graphe-rendu-compute.md).

## Contenu

| Fichier | Rôle |
|---------|------|
| [`render_graph.json`](render_graph.json) | Document v0 : ressources + passes (compute : buffers rw/ro, textures, **option** indirect + mips sur textures si étendu ; raster `hdr_color` + depth `scene_depth` ; **blit** avec ou sans `region`) — schéma [`docs/schemas/render-graph-v0.md`](../../docs/schemas/render-graph-v0.md). |
| [`b67_raster_depth_test.json`](b67_raster_depth_test.json) | Smoke **B.6/B.7** : seulement `raster_depth_mesh` + nœuds `ecs_before` / `ecs_after` (hôte natif) ; lire le test d’intégration. |
| [`shaders/`](shaders/) | WGSL référencés par le JSON (exécutés par `run_graph_v0_checksum` côté natif) ; y compris [`shadow_depth.wgsl`](shaders/shadow_depth.wgsl) (B.7). |
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

**Viewer PBR (B.4)** : depuis la racine `w3drs/`, le graphe s’encode dans le **même** `CommandEncoder` que le moteur (ordre ajustable) :

```bash
cargo run -p khronos-pbr-sample -- --render-graph fixtures/phases/phase-b/render_graph.json
cargo run -p khronos-pbr-sample -- --render-graph fixtures/phases/phase-b/render_graph.json --render-graph-slot post_pbr
```

- **`--render-graph-readback <id>`** si le readback n’est pas `hdr_color`
- **`--render-graph-slot`** : `pre` (avant Hi-Z), `after_cull` (défaut, après cull + copie indirect), `post_pbr` (après main PBR, avant tonemap)

## Suite

- Généraliser l’exécuteur (bindings, barrières explicites hors render pass) ; brancher le **même** JSON côté **GPU WASM** (B.5).
- Fusion pipeline principal (B.4 suite) : HDR / passes moteur pilotés par données.
