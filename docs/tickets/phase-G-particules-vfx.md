# Phase G — Particules & effets avancés

| Champ | Valeur |
|-------|--------|
| **ID** | `PHASE-G` |
| **Roadmap** | [ROADMAP § Phase G](../ROADMAP.md) |
| **Statut** | À faire |

## Axes prioritaires

- **Data-driven** : particle graph / émetteurs en **données** ; courbes d’émission versionnées.
- **Multithreading** : simulation compute sur GPU ; CPU game thread **coût quasi constant** — mesure sur N frames avec seuil.
- **Modularité** : module particules optionnel (feature) sans gonfler le renderer de base.

## Écart architecture (existant → cible)

- **Existant** : compute GPU pour culling etc. **sans** graphe particules auteur ni format dédié.
- **Cible** : **particle graph** + données ; cohérence éventuelle avec Hi-Z.
- **Ajustement** : ligne *particle graph* dans `architecture.md` mise à jour avec schéma + exemple.

## Périmètre

Compute simulation, indirect draw, tri/culling vs Hi-Z, ECS.


## Scène ou projet de test (validation fonctionnelle)

Chaque livraison doit inclure ou étendre **un projet de test versionné** permettant la **validation fonctionnelle** des fonctionnalités de cette phase (répétable, même sur CI dès que l’infra le permet).

| Champ | Valeur |
|-------|--------|
| **ID fixture** | phase-g |
| **Chemin cible** | fixtures/phases/phase-g/ (racine dépôt **w3drs/** ; créer au premier jalon — voir [convention](../../fixtures/phases/README.md)) |
| **Rôle** | Config particules / graphe + scène légère ; valide N particules, indirect, coût CPU game thread. |

### Évolution (ROADMAP)

| Moment | Attendu |
|--------|---------|
| **Aujourd’hui** | Dossier **scène / projet** + README.md : prérequis, commandes (cargo xtask client, www), point d’entrée (argument CLI, env, ou config). |
| **À terme** | **Workspace éditeur** + **.w3db** : chargement **natif** et **web** du **même** paquet pour QA sans divergence de données. |

### Description prescrite de la scène v0 (rédigée dès maintenant)

Spec de `fixtures/phases/phase-g/` : particules + compute.

| Élément | Contenu attendu |
|---------|-----------------|
| `README.md` | **N** particules cible ; FPS / coût CPU game thread à mesurer. |
| `particles.json` | Graphe ou config émetteur : taux, durée de vie, collision simple ; data-only. |
| `expected.md` | Compteur draw indirect / buffer alive ; stabilité sur M frames ; pas de fuite GPU (handles stables). |

### Critères de scène (DOR / DOD)

- **DOR** : [ ] fixtures/phases/phase-g/ décrit dans le README (ou PR) avec reproduction **documentée** ; hachages / LFS pour gros binaires.
- **DOD** : [ ] au moins un **test** (cargo test / E2E) **référence** ce chemin ; toute validation manuelle = **checklist** copiable dans la PR ; natif et web utilisent les **mêmes** assets lorsque les deux cibles sont dans le périmètre.


---

## Definition of Ready (DOR)

- [ ] **N cible** particules + FPS cible documentés (nombres entiers).
- [ ] Fichier graphe / config particules minimal en repo.
- [ ] Base CI verte.

---

## Definition of Done (DOD)

- [ ] Test charge : N particules, **temps CPU game thread** médian < seuil (µs/ms) sur ref ; **2 runs** ± tolérance documentée.
- [ ] Test GPU : indirect args ou buffer count stable attendu (assert).
- [ ] Intégration Hi-Z si applicable : test d’invariant réutilisant patterns `cull_integration`.

### Outils de validation

| Outil | Rôle | Seuil |
|-------|------|--------|
| `cargo test` | Simulation + invariants | 0 échec. |
| `criterion` ou instrumentation | CPU game thread | < seuil. |
| `wgpu` tests headless | Comptage / probe | Selon infra existante. |
| `xtask check` | wasm + native | Vert. |

---

## Journal

- [ ] [`../journal.md`](../journal.md) : N, seuils, résultats mesurés, liens shaders / data.
