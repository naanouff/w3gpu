# Fixture `phase-b` — attentes v0 (graphe déclaratif)

## Ordre des passes

1. `warmup_dispatch` — compute, `dispatch` = (4, 4, 1), shader `shaders/trivial_dispatch.wgsl`, entrée `cs_main`, `texture_reads` → `hdr_color`, `storage_buffers` → `indirect_args`, `storage_buffers_read` → `ro_pad`, `storage_buffers_group1` → `g1_side` (`@group(1)` rw), `storage_buffers_read_group1` → `g1_ro_pad` (`@group(1)` ro), `storage_writes` → texture `ping`.
2. `fullscreen_triangle_probe` — `raster_mesh`, shader `shaders/minimal_raster.wgsl`, `vs_main` / `fs_main`, cible couleur `hdr_color`, **depth** `scene_depth` (`Depth32Float`, même taille que `hdr_color`).
3. `fullscreen_second_pass` — **`fullscreen`** (même schéma / même encode que raster), même shader et cibles.
4. `mirror_hdr` — **`blit`** : `hdr_color` → `hdr_blit_dst` (même format / taille mip 0 ; `copy_src` / `copy_dst`). Variante équivalente : même blit avec un objet **`region`** couvrant entièrement le mip 0 (origines 0, `width`/`height` omis) — doit produire le **même** checksum qu’en intégration (`phase_b_graph_checksum_matches_when_blit_declares_full_mip0_region`).

## Validation automatisée

- `cargo test -p w3drs-render-graph` charge et valide [`render_graph.json`](render_graph.json) (schéma, version, ids uniques, dispatch non nul ; champs optionnels **indirect** / **region** / **mip_level_count** couverts par les tests du crate).
- `cargo test -p w3drs-renderer --test phase_b_graph_exec` exécute compute + raster sur `wgpu` et compare le **checksum** readback (`hdr_color`) sur **deux** frames identiques ; vérifie **compute indirect** (args semés = même résultat que dispatch fixe) ; cas shader manquant → `Io`.
- `cargo test -p w3drs-render-graph` — parse + **`validate_exec_v0`** (sans `wgpu`, même logique que la validation pré-GPU natif).
- **WASM** : `w3drsValidateRenderGraphV0` dans `www/pkg` après `npm run build:wasm`.
- `cargo run -p phase-b-graph --release` — binaire d’**illustration** (checksum sur **stderr**).

## Critère de sortie Phase B (ticket)

Lorsque l’**exécuteur** `wgpu` sera branché : deux exécutions consécutives sur graphe fixe → même checksum (buffer / image) ; graphes invalides → erreur typée (voir [phase-B-graphe-rendu-compute.md](../../docs/tickets/phase-B-graphe-rendu-compute.md)).
