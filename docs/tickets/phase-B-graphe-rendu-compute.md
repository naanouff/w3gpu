# Phase B — Graphe de rendu & compute

| Champ | Valeur |
|-------|--------|
| **ID** | `PHASE-B` |
| **Roadmap** | [ROADMAP § Phase B](../ROADMAP.md) |
| **Statut** | À faire |

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

- **DOR** : [ ] fixtures/phases/phase-b/ décrit dans le README (ou PR) avec reproduction **documentée** ; hachages / LFS pour gros binaires.
- **DOD** : [ ] au moins un **test** (cargo test / E2E) **référence** ce chemin ; toute validation manuelle = **checklist** copiable dans la PR ; natif et web utilisent les **mêmes** assets lorsque les deux cibles sont dans le périmètre.


---

## Definition of Ready (DOR)

- [ ] **Schéma** du format graphe (v0) versionné dans `docs/` ou `schemas/` avec numéro de version.
- [ ] **Démo data** : un fichier graphe minimal + shaders associés en repo (chemins relatifs stables).
- [ ] `cargo xtask check` vert sur la base.

---

## Definition of Done (DOD)

- [ ] Démo « compute + raster » pilotée **uniquement** par le fichier graphe (hors bootstrap documenté).
- [ ] Tests d’intégration : chargement graphe → ordre des passes déterministe ; **deux exécutions** consécutives produisent le même checksum GPU buffer (ou image de référence) sur scène de test fixe.
- [ ] Régression : graphes invalides rejetés avec erreur typée + code de validation testé.

### Outils de validation

| Outil | Rôle | Seuil |
|-------|------|--------|
| `cargo test` | Tests intégration graphe + exécuteurs | 0 échec. |
| `cargo llvm-cov` / tarpaulin | Couverture parse + exec | Seuil sur diff (PR). |
| `cargo xtask check` | wasm + native | Vert. |
| Client natif + WASM | Même graphe chargé sur **deux** cibles (deux jobs CI ou labels) | Tableau résultats OK / skip documenté. |
| Benchmark optionnel | `criterion` ou compteur passes/frame | Pas de régression > X % vs baseline enregistrée dans `journal.md`. |

---

## Journal

- [ ] Clôture → [`../journal.md`](../journal.md) : version schéma, lien démo, métriques parallélisme + checksum / image de référence.
