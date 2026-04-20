# Phase D — Format `.w3db` & streaming

| Champ | Valeur |
|-------|--------|
| **ID** | `PHASE-D` |
| **Roadmap** | [ROADMAP § Phase D](../ROADMAP.md) |
| **Statut** | À faire |

## Axes prioritaires

- **Data-driven** : le format **est** la donnée ; spec + golden files binaires versionnés ; migrations testées.
- **Multithreading** : IO + decode **async / pool** ; métrique **TTFP** et **max memory** sur scène de bench documentée.
- **Modularité** : crate `w3drs-pack` (nom indicatif) + CLI `xtask` ou binaire `w3db-tool` sans lier l’UI éditeur.

## Écart architecture (existant → cible)

- **Existant** : **pas** de format `.w3db` ni de bake workspace ; chargement scène = glTF direct ou démo.
- **Cible** : spec binaire + CLI + streaming **multithread** ; même paquet pour **compilateur** et runtime (voir `architecture.md` *Formats* + *Compilateur*).
- **Ajustement** : toute avancée spec → section **Formats** et table versions dans `architecture.md`.

## Périmètre

Spec binaire, runtime mmap/stream, CLI bake / validate / diff.


## Scène ou projet de test (validation fonctionnelle)

Chaque livraison doit inclure ou étendre **un projet de test versionné** permettant la **validation fonctionnelle** des fonctionnalités de cette phase (répétable, même sur CI dès que l’infra le permet).

| Champ | Valeur |
|-------|--------|
| **ID fixture** | phase-d |
| **Chemin cible** | fixtures/phases/phase-d/ (racine dépôt **w3drs/** ; créer au premier jalon — voir [convention](../../fixtures/phases/README.md)) |
| **Rôle** | Workspace minimal source + .w3db généré (ou golden) ; valide parse, stream, TTFP / mémoire. |

### Évolution (ROADMAP)

| Moment | Attendu |
|--------|---------|
| **Aujourd’hui** | Dossier **scène / projet** + README.md : prérequis, commandes (cargo xtask client, www), point d’entrée (argument CLI, env, ou config). |
| **À terme** | **Workspace éditeur** + **.w3db** : chargement **natif** et **web** du **même** paquet pour QA sans divergence de données. |

### Description prescrite de la scène v0 (rédigée dès maintenant)

Spec de `fixtures/phases/phase-d/` : **workspace source** + **`.w3db`** golden + métriques.

| Élément | Contenu attendu |
|---------|-----------------|
| `README.md` | Commande bake `workspace → .w3db` ; commande load runtime ; **TTFP** et RSS cibles chiffrées. |
| `workspace/` | Arborescence type [Goals.md](../Goals.md) **mini** (`assets/`, `src/`, …) uniquement données nécessaires au test. |
| `golden/*.w3db` | Paquet(s) de référence versionnés + numéro de **spec** embarqué. |
| `expected.md` | Entités / composants attendus après load partiel ou full ; erreurs invalides (fichiers corrompus) pour tests négatifs. |

### Critères de scène (DOR / DOD)

- **DOR** : [ ] fixtures/phases/phase-d/ décrit dans le README (ou PR) avec reproduction **documentée** ; hachages / LFS pour gros binaires.
- **DOD** : [ ] au moins un **test** (cargo test / E2E) **référence** ce chemin ; toute validation manuelle = **checklist** copiable dans la PR ; natif et web utilisent les **mêmes** assets lorsque les deux cibles sont dans le périmètre.


---

## Definition of Ready (DOR)

- [ ] Spec `.w3db` v0 rédigée (champs, endianness, versioning) + **exemples** hex ou blobs minuscules dans le repo.
- [ ] Scène de bench « moyenne » : taille totale cible + nombre d’entités **chiffrés**.
- [ ] Base CI verte.

---

## Definition of Done (DOD)

- [ ] `w3db-validate` (ou sous-commande `xtask`) retourne **code exit 0** sur golden valide et **non-zéro** sur corrompu (tests d’intégration sur stdin / fichiers tmp).
- [ ] **TTFP** et RSS max mesurés sur la scène de bench ; seuils inscrits dans la PR et dans `journal.md`.
- [ ] Tests **multithread** : N chargements concurrents sans data race (stress test reproductible, graine fixe).

### Outils de validation

| Outil | Rôle | Seuil |
|-------|------|--------|
| `cargo test` | Roundtrip bake → load → ECS state | Égalité structurée (snapshot serde ou hash d’état). |
| CLI validate | `cargo run -p … -- validate …` | Codes retour testés dans `tests/`. |
| `hyperfine` / script bench | TTFP, RSS | Seuils PR. |
| `cargo xtask check` | wasm + native | Vert. |
| `llvm-cov` | Couverture parse / stream | Seuil sur diff. |

---

## Journal

- [ ] [`../journal.md`](../journal.md) : version spec, liens CLI, **chiffres TTFP/RSS**, chemin scène bench.
