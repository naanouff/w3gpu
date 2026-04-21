# Phase E — Physique & interaction

| Champ | Valeur |
|-------|--------|
| **ID** | `PHASE-E` |
| **Roadmap** | [ROADMAP § Phase E](../ROADMAP.md) |
| **Statut** | À faire |

## Axes prioritaires

- **Data-driven** : mondes, layers, matériaux physiques, prefabs collision décrits par **données** ; pas de pile de cubes uniquement en dur dans `main` sans fichier scène.
- **Multithreading** : pas de **step** physique sur le thread render ; déport documenté + test de non-régression latence render.
- **Modularité** : backend (ex. Rapier) derrière **trait** `PhysicsWorld` dans crate dédié ; pas d’appels moteur de rendu depuis le crate physique.

## Écart architecture (existant → cible)

- **Existant** : **aucune** couche physique intégrée ni format de scène physique sérialisé.
- **Cible** : moteur physique derrière trait + **données** monde / triggers ; lien ticket + ligne *Graphes & simulation* dans `architecture.md`.
- **Ajustement** : documenter le format choisi (fichier dédié vs section `.w3db`) dans `architecture.md`.

## Périmètre

Intégration moteur physique, triggers ECS, navmesh ou équivalent.


## Scène ou projet de test (validation fonctionnelle)

Chaque livraison doit inclure ou étendre **un projet de test versionné** permettant la **validation fonctionnelle** des fonctionnalités de cette phase (répétable, même sur CI dès que l’infra le permet).

| Champ | Valeur |
|-------|--------|
| **ID fixture** | phase-e |
| **Chemin cible** | fixtures/phases/phase-e/ (racine dépôt **w3drs/** ; créer au premier jalon — voir [convention](../../fixtures/phases/README.md)) |
| **Rôle** | Scène physique data-driven (pile, sol, triggers) ; valide déterminisme + FPS de référence. |

### Évolution (ROADMAP)

| Moment | Attendu |
|--------|---------|
| **Aujourd’hui** | Dossier **scène / projet** + README.md : prérequis, commandes (cargo xtask client, www), point d’entrée (argument CLI, env, ou config). |
| **À terme** | **Workspace éditeur** + **.w3db** : chargement **natif** et **web** du **même** paquet pour QA sans divergence de données. |

### Description prescrite de la scène v0 (rédigée dès maintenant)

Spec de `fixtures/phases/phase-e/` : monde physique **data-driven**.

| Élément | Contenu attendu |
|---------|-----------------|
| `README.md` | Machine de référence 60 FPS ; graine RNG ; étapes de validation. |
| `scene.physics.json` | Sol, pile de N caisses, matériaux friction/restitution ; positions initiales. |
| `expected.md` | Positions finales à **t** fixe (epsilon) sur 2 runs **identiques** ; triggers : événements attendus. |

### Critères de scène (DOR / DOD)

- **DOR** : [ ] fixtures/phases/phase-e/ décrit dans le README (ou PR) avec reproduction **documentée** ; hachages / LFS pour gros binaires.
- **DOD** : [ ] au moins un **test** (cargo test / E2E) **référence** ce chemin ; toute validation manuelle = **checklist** copiable dans la PR ; natif et web utilisent les **mêmes** assets lorsque les deux cibles sont dans le périmètre.


---

## Definition of Ready (DOR)

- [ ] Choix moteur + version **gelés** dans `Cargo.toml` avec justification licence.
- [ ] Scène démo **fichier** (data) : pile d’objets + sol — paramètres numériques listés.
- [ ] Machine de référence pour 60 FPS **documentée** (CPU, OS, backend GPU).

---

## Definition of Done (DOD)

- [ ] Démo data-driven chargeable ; **60 FPS** sur machine de réf pour la scène démo (mesure sur N secondes, min FPS ≥ 60 ou seuil ajusté documenté).
- [ ] Tests : collision **déterministe** sur graine fixe (positions finales identiques sur 2 runs).
- [ ] WASM : stratégie threads / simd **écrite** + tests applicables (ou `skip` justifiés avec lien issue).

### Outils de validation

| Outil | Rôle | Seuil |
|-------|------|--------|
| `cargo test` | Déterminisme + triggers | Bit-exact ou epsilon documenté. |
| Script bench FPS | Mesure `khronos-pbr-sample` étendu ou binaire démo | ≥ 60 FPS min sur ref (PR). |
| `cargo clippy` / `xtask check` | Qualité | Vert. |
| Couverture | `llvm-cov` sur glue ECS / traits | Seuil diff. |

---

## Journal

- [ ] [`../journal.md`](../journal.md) : moteur, version, scène démo, **mesures FPS**, liens tests déterministes.
