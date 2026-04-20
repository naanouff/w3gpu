# Phase F — Terrain & géométrie procédurale

| Champ | Valeur |
|-------|--------|
| **ID** | `PHASE-F` |
| **Roadmap** | [ROADMAP § Phase F](../ROADMAP.md) |
| **Statut** | À faire |

## Axes prioritaires

- **Data-driven** : heightfields, règles LOD, graphes procéduraux en **fichiers** ; recompilation interdite pour changer une courbe LOD documentée.
- **Multithreading** : génération tuiles / cook CPU en jobs ; frame render sans stall mesurable > seuil.
- **Modularité** : terrain + GPROC en sous-systèmes branchables (features Cargo).

## Écart architecture (existant → cible)

- **Existant** : pas de terrain ni graphe procédural **data** dans le repo moteur.
- **Cible** : heightfield + graphe terrain / GPROC **fichiers** ; exécuteur topologique.
- **Ajustement** : compléter `architecture.md` (*Graphes & simulation → terrain*) quand le schéma v0 existe.

## Périmètre

Terrain LOD, GPROC SoA + exécuteur + nœuds MVP.


## Scène ou projet de test (validation fonctionnelle)

Chaque livraison doit inclure ou étendre **un projet de test versionné** permettant la **validation fonctionnelle** des fonctionnalités de cette phase (répétable, même sur CI dès que l’infra le permet).

| Champ | Valeur |
|-------|--------|
| **ID fixture** | phase-f |
| **Chemin cible** | fixtures/phases/phase-f/ (racine dépôt **w3drs/** ; créer au premier jalon — voir [convention](../../fixtures/phases/README.md)) |
| **Rôle** | Heightfield / règles LOD + graphe procédural ; valide terrain infini et reproductibilité hash géométrie. |

### Évolution (ROADMAP)

| Moment | Attendu |
|--------|---------|
| **Aujourd’hui** | Dossier **scène / projet** + README.md : prérequis, commandes (cargo xtask client, www), point d’entrée (argument CLI, env, ou config). |
| **À terme** | **Workspace éditeur** + **.w3db** : chargement **natif** et **web** du **même** paquet pour QA sans divergence de données. |

### Description prescrite de la scène v0 (rédigée dès maintenant)

Spec de `fixtures/phases/phase-f/` : terrain + procédural.

| Élément | Contenu attendu |
|---------|-----------------|
| `README.md` | Paramètres caméra ; LOD cible ; reproductibilité (graine). |
| `terrain/heightfield.png` (ou `.raw`) | Petite heightmap fixe ; métadonnées échelle dans `terrain_meta.json`. |
| `proc_graph.json` | Graphe GPROC minimal (primitives → merge) ; sortie attendue (nombre de verts / hash mesh). |
| `expected.md` | Hash géométrie ou nombre de triangles après cook ; budget ms cook par tuile. |

### Critères de scène (DOR / DOD)

- **DOR** : [ ] fixtures/phases/phase-f/ décrit dans le README (ou PR) avec reproduction **documentée** ; hachages / LFS pour gros binaires.
- **DOD** : [ ] au moins un **test** (cargo test / E2E) **référence** ce chemin ; toute validation manuelle = **checklist** copiable dans la PR ; natif et web utilisent les **mêmes** assets lorsque les deux cibles sont dans le périmètre.


---

## Definition of Ready (DOR)

- [ ] Référence fonctionnelle w3dts (UltraTerrain / GPROC) pointée par **commit** ou tag.
- [ ] **Fichiers data** de démo (terrain + graphe procédural minimal) dans le repo.
- [ ] Base CI verte.

---

## Definition of Done (DOD)

- [ ] Terrain « infini » démo avec paramètres **externes** ; scène procédurale chargée depuis data uniquement.
- [ ] Test reproductible : même graine → même mesh hash (ou nombre de vertices identique).
- [ ] Métrique : temps cook tuile médian < seuil (ms) sur machine ref (tableau PR).

### Outils de validation

| Outil | Rôle | Seuil |
|-------|------|--------|
| `cargo test` | Graphe procédural + hash géométrie | Déterministe. |
| Bench script | Cook tuiles | ms médian < seuil. |
| `cargo xtask check` | Cross-target | Vert. |
| Couverture | Topologie exécuteur GPROC | Seuil diff. |

---

## Journal

- [ ] [`../journal.md`](../journal.md) : formats data, seeds, métriques cook, captures si utiles.
