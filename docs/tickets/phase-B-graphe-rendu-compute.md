# Phase B — Graphe de rendu & compute

| Champ | Valeur |
|-------|--------|
| **ID** | `PHASE-B` |
| **Roadmap** | [ROADMAP § Phase B](../ROADMAP.md) |
| **Statut** | **En cours** — v0 + **B.1** (registre GPU) + **B.2** partiel (validation usages / passes, `pass_ids_in_order_v0`) ; **suite** : barrières **wgpu** explicites, fusion viewer, GPU **WASM** (voir *Plan d’exécution*) |

## Axes prioritaires

- **Data-driven** : graphe décrit par **fichier versionné** (JSON/RON/binaire) ; aucune passe obligatoire uniquement hardcodée hors données de référence minimale bootstrap documentée.
- **Multithreading** : compilation / validation du graphe et uploads en **hors thread render** ; métrique : temps CPU blocage render < seuil (ms) fixé en PR.
- **Modularité** : crate ou module `render-graph` (nom à trancher) consommé par `w3drs-renderer` sans coupler l’éditeur.

## Écart architecture (existant → cible)

- **Existant** : passes **codées** dans `w3drs-renderer` ; compute Hi-Z / cull couplés au moteur, **pas** de graphe déclaratif fichier.
- **Cible** : **render graph** + ressources décrites en **JSON/RON** ; exécuteurs génériques ; ECS relié au graphe via data.
- **Ajustement** : toute nouvelle passe doit **exister dans le fichier graphe** du fixture `phase-b` ou justifier un bootstrap minimal documenté dans `architecture.md` (*Diagrammes / pipeline*).

## Périmètre

Description déclarative, registre ressources, exécuteurs raster/fullscreen/compute, intégration ECS.


## Scène ou projet de test (validation fonctionnelle)

Chaque livraison doit inclure ou étendre **un projet de test versionné** permettant la **validation fonctionnelle** des fonctionnalités de cette phase (répétable, même sur CI dès que l’infra le permet).

| Champ | Valeur |
|-------|--------|
| **ID fixture** | phase-b |
| **Chemin cible** | fixtures/phases/phase-b/ (racine dépôt **w3drs/** ; créer au premier jalon — voir [convention](../../fixtures/phases/README.md)) |
| **Rôle** | Fichier graphe de rendu déclaratif + shaders ; démo compute+raster pilotée par data sans fork moteur. |

### Évolution (ROADMAP)

| Moment | Attendu |
|--------|---------|
| **Aujourd’hui** | Dossier **scène / projet** + README.md : prérequis, commandes (cargo xtask client, www), point d’entrée (argument CLI, env, ou config). |
| **À terme** | **Workspace éditeur** + **.w3db** : chargement **natif** et **web** du **même** paquet pour QA sans divergence de données. |

### Description prescrite de la scène v0 (rédigée dès maintenant)

Spec de `fixtures/phases/phase-b/` : tout le rendu + compute piloté par **données**, sans fork moteur.

| Élément | Contenu attendu |
|---------|-----------------|
| `README.md` | Comment charger le graphe sous **natif** et **web** ; critère de succès (image ou buffer attendu). |
| `render_graph.json` | Ressources (buffers / textures) + passes **raster** + **au moins une** passe **compute** (ex. gradient ou simu minimale) + dispatch. |
| `shaders/*.wgsl` | Entrypoints nommés comme dans le JSON ; chemins relatifs. |
| `expected.md` | Ordre des passes **déterministe** ; métrique (checksum buffer / taille mip / compteur draw indirect). |

### Critères de scène (DOR / DOD)

- **DOR** : [x] `fixtures/phases/phase-b/` + README + graphe + shaders (pas de gros binaire ; LFS N/A v0).
- **DOD** : [x] exécuteur GPU natif + checksum ; [x] **test** : `cargo test -p w3drs-render-graph` (parse + `validate_exec_v0` + fixture) ; [x] **web** : `w3drsValidateRenderGraphV0` dans `www/pkg` (validation JSON, pas encore run GPU) ; [ ] validation manuelle visuelle.


---

## Definition of Ready (DOR)

- [x] **Schéma** du format graphe (v0) : [`docs/schemas/render-graph-v0.md`](../schemas/render-graph-v0.md).
- [x] **Démo data** : [`fixtures/phases/phase-b/render_graph.json`](../../fixtures/phases/phase-b/render_graph.json) + [`shaders/`](../../fixtures/phases/phase-b/shaders/).
- [x] `cargo xtask check` vert sur la base (à réexécuter après chaque merge touchant le workspace).

---

## Definition of Done (DOD)

- [x] Démo « compute + raster » pilotée par le fichier graphe + shaders fixture (exécuteur minimal `w3drs_renderer::render_graph_exec::run_graph_v0_checksum`, hors intégration au viewer principal).
- [x] Tests d’intégration : `cargo test -p w3drs-renderer --test phase_b_graph_exec` — **deux** exécutions consécutives → même checksum readback texture `hdr_color`.
- [x] Régression parse : `cargo test -p w3drs-render-graph` + `RenderGraphError` testé (schéma invalide, etc.).
- [x] Régression exécuteur : `w3drs_render_graph::validate_exec_v0` + tests dans `exec_validate.rs` ; `validate_render_graph_exec_v0` = délégation + `From` → `RenderGraphExecError` ; test intégration **`phase_b_graph_missing_shader_returns_io`** (`Io`).

### Outils de validation

| Outil | Rôle | Seuil |
|-------|------|--------|
| `cargo test` | Tests intégration graphe + exécuteurs | 0 échec. |
| `cargo llvm-cov` / tarpaulin | Couverture parse + exec | Seuil sur diff (PR). |
| `cargo xtask check` | wasm + native | Vert. |
| Client natif + WASM | Même graphe chargé sur **deux** cibles (deux jobs CI ou labels) | Tableau résultats OK / skip documenté. |
| Benchmark optionnel | `criterion` ou compteur passes/frame | Pas de régression > X % vs baseline enregistrée dans `journal.md`. |

---

## Plan d’exécution — exécuteur complet & WASM (cible w3dts)

Objectif : même **souplesse** qu’un *RenderGraph* w3dts (fichier + passes raster/compute) **sans fork** du moteur, **y compris** dans le **navigateur** une fois l’exécuteur complet branché.

| Jalon | Livrable | Rôle |
|-------|----------|------|
| B.1 | **Registre de ressources** (buffers/textures, resize, noms) | **Partiel (2026-04-20)** : `RenderGraphGpuRegistry` + look-up + `resize_texture_2d` + `run_graph_v0_checksum_with_registry` ; **reste** : alias, cohérence tailles doc ↔ GPU dans le schéma. |
| B.2 | **Barrières / synchronisation** entre passes (read/write, render vs compute) | **Partiel** : validation **sans GPU** + `pass_ids_in_order_v0` ; exécuteur natif : **depth** sur `raster_mesh` (`depth_target` → render pass + pipeline depth, clear profondeur ; stencil si format combo) ; **reste** : barrières explicites hors render pass, DAG, annotations fines. |
| B.3 | **Généralisation** des exécutions `wgpu` : fullscreen, compute indirect quand le schéma le supporte, bind layouts **data** | Évite que chaque feature ajoute un chemin *ad hoc* dans `w3drs-renderer`. |
| B.4 | **Fusion** avec le **viewer PBR** (`RenderState` / loop actuelle) : le graphe **remplace** ou **configure** le pipeline principal pour un projet donné. |
| B.5 | **WASM** : même *encode* que B.1–B.3 sur `Device/Queue` du canvas WebGPU ; tests smoke `www/` + `cargo check` wasm. |
| B.6 | **ECS** : nœuds ↔ systèmes (préparation d’`entity_indirect_buf` ou uniforms entre passes), spec à figer. |

*Référence normative* : [schéma v0 — feuille de route](../schemas/render-graph-v0.md#feuille-de-route--exécuteur-complet-parité-moteur-objectif-w3dts).

**État (2026-04)** : B.0 **fait** ; **B.1** **amorcé** ; **B.2** **amorcé** (validation statique + ordre des passes) ; B.3–B.6 **ouverts** — DOD *client natif + WASM* (même JSON exécuté GPU web) reste *skip* tant que B.5 n’est pas clos.

---

## Journal

- [x] **2026-04-20** — jalon parse : crate `w3drs-render-graph`, spec v0, fixture `phase-b` ; **B.1** : `RenderGraphGpuRegistry`, `resize_texture_2d`, `run_graph_v0_checksum_with_registry` ; tests `phase_b_graph_exec` (resize + buffer + id inconnu) ; **B.2** : champs optionnels `texture_reads` / `storage_writes` sur `compute`, validation usages par passe, doublons `color_targets`, `depth_target` + formats depth ; `pass_ids_in_order_v0` ; détail [`../journal.md`](../journal.md).
- [x] **2026-04-24** — *Plan d’exécution* (jalons B.1–B.6) + renvois [schéma v0 — feuille de route](../schemas/render-graph-v0.md#feuille-de-route--exécuteur-complet-parité-moteur-objectif-w3dts) ; voir [`../journal.md`](../journal.md) section *Documentation — port 1:1…*.
- [ ] Clôture (Phase B « complète » 1:1) → exécuteur fusion viewer + GPU WASM + [`../journal.md`](../journal.md) : métriques parallélisme + image de référence.
