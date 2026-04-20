# Phase L — Industrialisation « prod »

| Champ | Valeur |
|-------|--------|
| **ID** | `PHASE-L` |
| **Roadmap** | [ROADMAP § Phase L](../ROADMAP.md) |
| **Statut** | À faire |

## Axes prioritaires

- **Data-driven** : configs CI, profils coverage, matrices build en **fichiers** (YAML workflows, `deny.toml`) versionnés — pas de seuils magiques uniquement dans UI CI non versionnée.
- **Multithreading** : jobs CI parallèles documentés ; tests parallèles `cargo nextest` (optionnel) avec nombre de threads fixe pour reproductibilité.
- **Modularité** : publication crates / npm **séparée** ; features optionnelles bien découpées.

## Écart architecture (existant → cible)

- **Existant** : `xtask check`, tests crates, hooks ; **pas** encore E2E systématiques, coverage seuils, ni **profils build** alignés compilateur.
- **Cible** : CI = **même** chaîne que le compilateur (README tickets + `architecture.md` *Compilateur*) ; `cargo deny`, rapports coverage.
- **Ajustement** : documenter les jobs et **seuils** dans `architecture.md` ou ADR ; tenir `phase-l` fixture **ci_smoke** à jour.

## Périmètre

CI, couverture, E2E, client natif en CI, semver, `cargo deny`, changelog, artefacts.


## Scène ou projet de test (validation fonctionnelle)

Chaque livraison doit inclure ou étendre **un projet de test versionné** permettant la **validation fonctionnelle** des fonctionnalités de cette phase (répétable, même sur CI dès que l’infra le permet).

| Champ | Valeur |
|-------|--------|
| **ID fixture** | phase-l |
| **Chemin cible** | fixtures/phases/phase-l/ (racine dépôt **w3drs/** ; créer au premier jalon — voir [convention](../../fixtures/phases/README.md)) |
| **Rôle** | Méta : scripts CI qui invoquent **plusieurs** fixtures/phases/phase-* (smoke agrégé) + seuils coverage ; pas une scène graphique unique. |

### Évolution (ROADMAP)

| Moment | Attendu |
|--------|---------|
| **Aujourd’hui** | Dossier **scène / projet** + README.md : prérequis, commandes (cargo xtask client, www), point d’entrée (argument CLI, env, ou config). |
| **À terme** | **Workspace éditeur** + **.w3db** : chargement **natif** et **web** du **même** paquet pour QA sans divergence de données. |

### Description prescrite de la scène v0 (rédigée dès maintenant)

`fixtures/phases/phase-l/` n’est **pas** une scène graphique unique : c’est le **paquet de smoke CI** qui **référence** les autres fixtures.

| Élément | Contenu attendu |
|---------|-----------------|
| `README.md` | Liste des phases `phase-a` … **invoquées** dans l’ordre par le pipeline CI ; machines runners. |
| `ci_smoke.yaml` | Matrice : jobs, timeouts, seuils coverage, flags `skip` GPU. |
| `expected.md` | Tous les codes retour attendus ; artefacts (logs, rapports) déposés en CI. |

### Critères de scène (DOR / DOD)

- **DOR** : [ ] fixtures/phases/phase-l/ décrit dans le README (ou PR) avec reproduction **documentée** ; hachages / LFS pour gros binaires.
- **DOD** : [ ] au moins un **test** (cargo test / E2E) **référence** ce chemin ; toute validation manuelle = **checklist** copiable dans la PR ; natif et web utilisent les **mêmes** assets lorsque les deux cibles sont dans le périmètre.


---

## Definition of Ready (DOR)

- [ ] Liste des **jobs CI** actuels + gaps documentés (tableau).
- [ ] Seuils coverage **chiffrés** proposés (statement / diff) alignés CONTRIBUTING.
- [ ] Politique semver 0.x **écrite** dans CONTRIBUTING ou ADR pointée.

---

## Definition of Done (DOD)

- [ ] Workflow CI exécute : `fmt`, `clippy -D warnings`, `test` workspace, `wasm32` check, **couverture** avec **seuil** (fail si sous seuil sur crates listés).
- [ ] **E2E** `www/` : au moins un scénario **thirtyfour** ou **chromiumoxide** vert (ou issue « P0 » si reportée avec plan).
- [ ] **Client natif** : job ou test documenté exécuté sur runner avec GPU / software stack connu (ou `skip` explicite + test headless obligatoire).
- [ ] `cargo deny check` (ou équivalent) vert ; advisories traités ou justifiés dans PR.
- [ ] `CHANGELOG.md` ou section journal pour **release** si applicable.

### Outils de validation

| Outil | Rôle | Seuil |
|-------|------|--------|
| GitHub Actions / CI | Pipeline complète | Vert sur branche merge. |
| `cargo llvm-cov` / `tarpaulin` | Couverture agrégée | ≥ seuil (PR). |
| `thirtyfour` / `chromiumoxide` | E2E `www/` | 1+ scénario vert. |
| `cargo deny` | Licences / advisories | 0 erreur bloquante. |
| `cargo nextest` (optionnel) | Parallélisme tests stable | Config versionnée. |
| `npm` / `pnpm` dans CI | Tests TS `www/` si Vitest ajouté | 0 échec. |

---

## Journal

- [ ] [`../journal.md`](../journal.md) : version outillage CI, seuils coverage, lien workflow, décisions `deny` / semver.
