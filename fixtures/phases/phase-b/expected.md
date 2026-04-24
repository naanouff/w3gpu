# Fixture `phase-b` — attentes v0 (graphe déclaratif)

## Ordre des passes

1. `warmup_dispatch` — compute, `dispatch` = (4, 4, 1), shader `shaders/trivial_dispatch.wgsl`, entrée `cs_main`.
2. `fullscreen_triangle_probe` — raster mesh, shader `shaders/minimal_raster.wgsl`, `vs_main` / `fs_main`, cible couleur `hdr_color`, **depth** `scene_depth` (`Depth32Float`, même taille que `hdr_color`).

## Validation automatisée

- `cargo test -p w3drs-render-graph` charge et valide [`render_graph.json`](render_graph.json) (schéma, version, ids uniques, dispatch non nul).
- `cargo test -p w3drs-renderer --test phase_b_graph_exec` exécute compute + raster sur `wgpu` et compare le **checksum** readback (`hdr_color`) sur **deux** frames identiques (+ cas shader manquant → `Io`).
- `cargo test -p w3drs-render-graph` — parse + **`validate_exec_v0`** (sans `wgpu`, même logique que la validation pré-GPU natif).
- **WASM** : `w3drsValidateRenderGraphV0` dans `www/pkg` après `npm run build:wasm`.
- `cargo run -p phase-b-graph --release` — binaire d’**illustration** (checksum sur **stderr**).

## Critère de sortie Phase B (ticket)

Lorsque l’**exécuteur** `wgpu` sera branché : deux exécutions consécutives sur graphe fixe → même checksum (buffer / image) ; graphes invalides → erreur typée (voir [phase-B-graphe-rendu-compute.md](../../docs/tickets/phase-B-graphe-rendu-compute.md)).
