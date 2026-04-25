# Phase B — Graphe de rendu & compute

| Champ | Valeur |
|-------|--------|
| **ID** | `PHASE-B` |
| **Roadmap** | [ROADMAP § Phase B](../ROADMAP.md) |
| **Statut** | **Terminée (v0)** — B.1–B.7 : parse/validation, exécuteur **natif + WASM**, `raster_depth_mesh` + hôte **B.7**, nœuds **`ecs_before` / `ecs_after`** + hôte **B.6** ; fixture **phase-b** + [`b67_raster_depth_test.json`](../../fixtures/phases/phase-b/b67_raster_depth_test.json) ; intégration **khronos** + slot. **Poursuite** (hors clôture fonctionnelle) : barrières wgpu explicites, remplacer des passes moteur par ressources injectées — [§ Poursuites](#poursuites-hors-périmètre-v0) |

## Axes prioritaires

- **Data-driven** : graphe décrit par **fichier versionné** (JSON/RON/binaire) ; aucune passe obligatoire uniquement hardcodée hors données de référence minimale bootstrap documentée.
- **Multithreading** : compilation / validation du graphe et uploads en **hors thread render** ; métrique : temps CPU blocage render < seuil (ms) fixé en PR.
- **Modularité** : crate ou module `render-graph` (nom à trancher) consommé par `w3drs-renderer` sans coupler l’éditeur.

## Écart architecture (existant → cible)

- **Existant** : passes **codées** dans `w3drs-renderer` ; compute Hi-Z / cull couplés au moteur, **pas** de graphe déclaratif fichier.
- **Cible** : **render graph** + ressources décrites en **JSON/RON** ; exécuteurs génériques ; ECS relié au graphe via data.
- **Ajustement** : toute nouvelle passe doit **exister dans le fichier graphe** du fixture `phase-b` ou justifier un bootstrap minimal documenté dans `architecture.md` (*Diagrammes / pipeline*).

## Périmètre

Description déclarative, registre ressources, exécuteurs raster/fullscreen/compute, intégration ECS (labels + hôte).

### Types de passes — v0

| `kind` (JSON) | Statut | Rôle (résumé) |
|---------------|--------|---------------|
| `compute` | **v0** | Dispatch fixe ou **`indirect_dispatch`** ; bindings g0 + g1 ; `ecs_before` / `ecs_after` (B.6). |
| `raster_mesh` | **v0** | + `ecs_*` (B.6). |
| `fullscreen` | **v0** | idem. |
| `raster_depth_mesh` | **v0 (B.7)** | Profondeur seule, layout `shadow_depth` ; `draw_raster_depth_mesh` hôte. |
| `blit` | **v0** | + `ecs_*` (B.6). |
| *Autres* | **Backlog** | *clear* MSAA, *resolve*, *copy_to_buffer* — [schéma — évolutions](../schemas/render-graph-v0.md#évolutions-futures-hors-v0). |

**Spec (2026)** : `fullscreen` est un **`kind` distinct** avec le même schéma que `raster_mesh` en v0 (validation partagée, encode identique côté natif).

## Scène ou projet de test (validation fonctionnelle)

| Champ | Valeur |
|-------|--------|
| **ID fixture** | phase-b |
| **Chemin cible** | fixtures/phases/phase-b/ |
| **Rôle** | Fichier graphe de rendu déclaratif + shaders ; démo compute+raster pilotée par data sans fork moteur. |

### Critères de scène (DOR / DOD)

- **DOR** : [x] `fixtures/phases/phase-b/` + README + graphe + shaders.
- **DOD** : [x] exécuteur GPU + tests ; [x] web : validation + `w3drsPhaseBGraphRunChecksum` ; [x] B.6/B.7 : test `phase_b_b6_b7_raster_depth_mesh_host_hooks_and_depth_encode` + [`b67_raster_depth_test.json`](../../fixtures/phases/phase-b/b67_raster_depth_test.json).

---

## Definition of Ready (DOR)

- [x] **Schéma** v0 : [`docs/schemas/render-graph-v0.md`](../schemas/render-graph-v0.md).
- [x] **Démo data** : [`render_graph.json`](../../fixtures/phases/phase-b/render_graph.json) + [`shaders/`](../../fixtures/phases/phase-b/shaders/).
- [x] `cargo xtask check` vert sur la base.

---

## Definition of Done (DOD)

- [x] Démo « compute + raster » pilotée par le fichier graphe + shaders.
- [x] Tests d’intégration : `phase_b_graph_exec` (y compris indirect, blit région, **B.6/B.7** hôte).
- [x] Régression parse : `w3drs-render-graph`.
- [x] `validate_render_graph_exec_v0` + intégration I/O.
- [x] **B.6** : champs `ecs_before` / `ecs_after` + `RenderGraphV0Host` ; **B.7** : `raster_depth_mesh` + `draw_raster_depth_mesh` + `insert_buffer` / `insert_texture_2d`.

---

## Plan d’exécution — exécuteur complet & WASM (cible w3dts)

| Jalon | Livrable | Rôle |
|-------|----------|------|
| B.1 | **Registre** | v0 : `RenderGraphGpuRegistry` + `insert_buffer` / `insert_texture_2d` (B.7) ; resize, mips. |
| B.2 | **Barrières** | v0 : validation + ordre + depth raster ; *suite* : barrières wgpu explicites. |
| B.3 | **Passes** | v0 : compute, fullscreen, blit, indirect, mips. |
| B.4 | **Viewer** | `khronos-pbr-sample` — emplacement d’encodage. |
| B.5 | **WASM** | encode + checksum ; hôte `Noop` par défaut. |
| B.6 | **ECS** | v0 : labels `ecs_before` / `ecs_after` → `ecs_node` (exé. systèmes côté hôte natif). |
| B.7 | **Shadow** data-driven | v0 : `raster_depth_mesh` + hôte `draw_raster_depth_mesh` (même sémantique que [`ShadowPass`](../../crates/w3drs-renderer/src/shadow_pass.rs) une fois câblé). |

*Référence* : [schéma v0](../schemas/render-graph-v0.md).

**État (v0, 2026-04-24+)** : B.1–B.7 livrés au sens **data + exécuteur + hôtes** ; câblage ombre *réel* du viewer PBR sur le JSON = **poursuite** (identifiants ressources moteur ↔ noms du graphe).

---

## Poursuites (hors périmètre *schéma v0 figé*)

- Barrières explicites hors pass intégré, DAG.
- Câblage scène : HDR / depth moteur ↔ mêmes ids que le JSON.
- *Editor* : sélection nœud ECS par UI (au-delà de chaînes dans le JSON).

---

## Journal (extrait)

- [x] **2026-04-24+** — **B.6 / B.7 intégrés v0** : `raster_depth_mesh`, `RenderGraphV0Host`, `encode_render_graph_passes_v0_with_wgsl_host`, `run_graph_v0_checksum_with_registry_wgsl_host` ; test `phase_b_b6_b7_…` ; [schéma v0](../schemas/render-graph-v0.md) ; [journal](../journal.md).
- [x] **Passes** : `kind` `fullscreen` + `blit` (v0) ; **évolutions** : *clear*, *resolve* — [schéma v0 — futures](../schemas/render-graph-v0.md#évolutions-futures-hors-v0).
