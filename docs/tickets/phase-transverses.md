# Ticket transverse — Priorités cadencement (toutes phases)

| Champ | Valeur |
|-------|--------|
| **ID** | `PHASE-X-TRANS` |
| **Type** | Gate permanent / revue par PR |
| **Référence** | [ROADMAP — Principes](../ROADMAP.md) |

## Objectif

Formaliser le contrôle **répétable** des trois priorités : **data-driven**, **multithreading**, **modularité** (moteur ↔ éditeur), pour toute livraison touchant le dépôt w3drs.

**Écart global** : synthèse **existant → cible** dans [README tickets — Écart existant vs cible](README.md#écart-existant-vs-cible-architecturale) et détail dans [`architecture.md`](../architecture.md).

---

## Definition of Ready (DOR)

- [ ] La PR / le ticket cite **quel axe transverse** est impacté (un ou plusieurs).
- [ ] Si **data-driven** : lien vers le fichier de **données** ou schéma (pas seulement du code Rust modifié sans artefact data).
- [ ] Si **multithreading** : description du **thread / job** propriétaire de chaque mutation partagée ; pas de nouveau `std::thread::spawn` non documenté sur le hot path render sans revue.
- [ ] Si **modularité** : liste des **crates** ou **features** touchés ; pas de dépendance circulaire nouvelle.
- [ ] **Scène / projet de test** : si la PR livre une capacité couverte par une phase A–L, elle **met à jour** le dossier `fixtures/phases/<phase-id>/` concerné (ou en crée un) conformément au ticket de phase ; sinon lien vers le fixture existant utilisé pour la revue.
- [ ] **Écart architecture** : la PR cite la sous-section **« Écart architecture »** du ticket de phase concerné et précise **quel écart** du tableau [README tickets](README.md#écart-existant-vs-cible-architecturale) est réduit (ou justifie l’exception).

---

## Definition of Done (DOD)

- [ ] **Data-driven** : aucune règle métier ou disposition **uniquement** codée en dur qui relève du périmètre ROADMAP *data-driven* ; exceptions listées dans la PR avec lien ROADMAP.
- [ ] **Multithreading** : tests ou benchmarks montrant l’absence de régression sur le modèle parallèle (ex. `cargo test` sur crates ECS + tests stress documentés si applicable).
- [ ] **Modularité** : `cargo check --workspace` ; frontière moteur/UI respectée (pas d’import UI dans `w3drs-renderer` sans feature gate).
- [ ] **`docs/journal.md`** mis à jour si la PR clôt un jalon transverse majeur (nouvelle politique, nouveau crate).
- [ ] **Fixture** : tout nouveau chemin sous `fixtures/phases/` est référencé dans la PR et **exploité** par au moins un test ou une checklist reproductible.
- [ ] **Documentation d’architecture** : si la PR change une limite « existant vs cible », **`docs/architecture.md`** (ou le ticket de phase) est mis à jour pour refléter le **nouvel état** (écart résiduel explicite).

### Outils de validation

| Outil | Rôle mesurable |
|-------|----------------|
| `cargo xtask check` | Native + `wasm32-unknown-unknown` sans erreur. |
| `cargo clippy --workspace -- -D warnings` | Zéro warning. |
| `cargo test -p w3drs-math -p w3drs-ecs -p w3drs-assets` (+ crates ajoutés par la PR) | Tous les tests verts, reproductibles. |
| `cargo fmt --check` | Style conforme. |
| Revue checklist CONTRIBUTING | Case **Tests** + **client natif** si applicable. |

---

## Journal

À chaque **changement de politique** transverse approuvé : entrée dans [`../journal.md`](../journal.md) (date + décision + lien PR).
