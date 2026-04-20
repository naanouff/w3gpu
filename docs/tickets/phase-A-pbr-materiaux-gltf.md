# Phase A — Parité rendu moteur (PBR + matériaux + glTF)

| Champ | Valeur |
|-------|--------|
| **ID** | `PHASE-A` |
| **Roadmap** | [ROADMAP § Phase A](../ROADMAP.md) |
| **Statut** | À faire |

## Axes prioritaires

- **Data-driven** : extensions matériau / variantes pipeline décrites par **données** (tables, JSON/RON, clés de shader) ; assets Khronos en **fixtures** versionnées.
- **Multithreading** : décodage / upload textures et préparation CPU en **jobs** sans bloquer le thread render ; mesurable via traces ou compteurs de frames bloquées = 0.
- **Modularité** : extensions glTF et tables de matériaux isolées dans `w3drs-assets` (ou crate dédié `w3drs-gltf-ext` si nécessaire) sans entasser la logique dans un seul module monolithique.

## Écart architecture (existant → cible)

- **Existant** : PBR + glTF **dans le code** (`w3drs-assets` / renderer) ; extensions Khronos **incomplètes** ; pas de manifeste centralisé de matériaux pour les fixtures.
- **Cible** : extensions + variantes pilotées par **données** ; shortlist Khronos + `materials/` comme dans la [description de scène](#description-prescrite-de-la-scène-v0-rédigée-dès-maintenant) ; alignement gates w3dts.
- **Ajustement** : chaque PR indique **quel hardcode** disparaît au profit de quel fichier data ; mise à jour de [`architecture.md`](../architecture.md) section *Formats* si nouvelle règle d’import / matériau.

## Périmètre (rappel)

Extensions `KHR_materials_*`, pipeline matériaux versionné, stratégie shader (A1 WGSL / A2 shader graph), régression assets Khronos.


## Scène ou projet de test (validation fonctionnelle)

Chaque livraison doit inclure ou étendre **un projet de test versionné** permettant la **validation fonctionnelle** des fonctionnalités de cette phase (répétable, même sur CI dès que l’infra le permet).

| Champ | Valeur |
|-------|--------|
| **ID fixture** | phase-a |
| **Chemin cible** | fixtures/phases/phase-a/ (racine dépôt **w3drs/** ; créer au premier jalon — voir [convention](../../fixtures/phases/README.md)) |
| **Rôle** | Shortlist glTF Khronos + données matériaux / variantes pipeline ; valide extensions PBR livrées (natif + web). |

### Évolution (ROADMAP)

| Moment | Attendu |
|--------|---------|
| **Aujourd’hui** | Dossier **scène / projet** + README.md : prérequis, commandes (cargo xtask client, www), point d’entrée (argument CLI, env, ou config). |
| **À terme** | **Workspace éditeur** + **.w3db** : chargement **natif** et **web** du **même** paquet pour QA sans divergence de données. |

### Description prescrite de la scène v0 (rédigée dès maintenant)

Cette section est la **spec de contenu** de `fixtures/phases/phase-a/` (dossier à matérialiser au fil des PR ; déjà contractuelle pour revue).

| Élément | Contenu attendu |
|---------|-----------------|
| `README.md` | Prérequis GPU / WebGPU ; commandes `cargo xtask client …` et `www/` ; **checklist visuelle** par asset (5–15 items). |
| `manifest.json` | Liste ordonnée des GLB : **DamagedHelmet** (réutilisation `www/public` possible) + slots optionnels `KHR_materials_anisotropy`, `KHR_materials_ior`, etc. ; chemins **relatifs** au fixture. |
| `glb/` | Binaires + **SHA256** (ou Git LFS + id objet) pour reproductibilité CI. |
| `materials/*.ron` ou `*.json` | Paramètres / variantes pipeline **data-driven** (évite constantes uniquement dans le Rust des tests). |
| `expected.md` | Critères **mesurables** (ex. pas de fallback rose sur MR ; highlights anisotropes visibles sous lumière orbitale). |

**Scène minimale** : sphère PBR témoin + un asset « gate » Khronos dans le même chargement pour une passe de validation unique.

### Critères de scène (DOR / DOD)

- **DOR** : [ ] fixtures/phases/phase-a/ décrit dans le README (ou PR) avec reproduction **documentée** ; hachages / LFS pour gros binaires.
- **DOD** : [ ] au moins un **test** (cargo test / E2E) **référence** ce chemin ; toute validation manuelle = **checklist** copiable dans la PR ; natif et web utilisent les **mêmes** assets lorsque les deux cibles sont dans le périmètre.


---

## Definition of Ready (DOR)

- [ ] **Shortlist d’assets** Khronos (noms + versions) listée dans ce fichier ou dans `docs/` lié ; hashes SHA256 des fichiers utilisés en CI (ou Git LFS ids).
- [ ] **Checklist PBR** alignée w3dts (lignes de critères de done) copiée ou référencée par URL commit w3dts.
- [ ] Branche de base : `cargo xtask check` **vert** sur le commit parent.

---

## Definition of Done (DOD)

- [ ] Chaque extension / chemin matériau livré a des **tests** qui exécutent les branches (couverture sur le diff ; voir CONTRIBUTING).
- [ ] **Régression** : jeu minimal d’assets Khronos rendus sans artefact bloquant défini dans la checklist ; captures golden **optionnelles** mais si présentes : comparaison binaire ou SSIM seuil documenté.
- [ ] **WASM + natif** : même matériau testé ou justifié par matrice de test (tableau PR).

### Outils de validation

| Outil | Commande / artefact | Seuil mesurable |
|-------|---------------------|-----------------|
| Tests unitaires / intégration | `cargo test -p w3drs-assets -p w3drs-renderer` (+ crates modifiés) | 0 échec ; temps wall-clock documenté sur CI self-hosted si GPU. |
| Couverture | `cargo llvm-cov test --workspace --exclude w3drs-wasm` (ou `tarpaulin`) sur crates touchés | Seuil minimal sur **lignes du diff** (ex. ≥ 85 % des lignes ajoutées) — chiffre exact figé dans la PR. |
| Cible WASM | `cargo test -p w3drs-wasm` si tests wasm-bindgen ; sinon `cargo check -p w3drs-wasm --target wasm32-unknown-unknown` | Vert. |
| Client natif | `cargo test` incluant scénarios `wgpu` headless existants **étendus** pour nouveaux layouts ; `cargo xtask client` smoke manuel documenté (checklist 5 étapes max) | Pas de panic ; pas de validation layer error sur assets de test. |
| fmt / clippy | `cargo fmt --check` ; `cargo clippy --workspace -- -D warnings` | Vert. |

---

## Journal

- [ ] À la clôture : mise à jour de [`../journal.md`](../journal.md) (sous-section **Phase A** ou entrée datée) : extensions livrées, outils utilisés, liens PR, **résultats chiffrés** (couverture, liste assets validés).
