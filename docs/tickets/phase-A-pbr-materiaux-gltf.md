# Phase A — Parité rendu moteur (PBR + matériaux + glTF)

| Champ | Valeur |
|-------|--------|
| **ID** | `PHASE-A` |
| **Roadmap** | [ROADMAP § Phase A](../ROADMAP.md) |
| **Statut** | **Terminée** (périmètre checklist Phase A + gates enregistrés) — [phase-a-gates-record.md](phase-a-gates-record.md). **Moyen terme** PBR (transmission avancée, AO, sheen, matériaux au-delà de `default.json`, A2…) : hors clôture ; voir [§ Moyen terme](#moyen-terme--rigueur-pbr--prod) et [ROADMAP § Phase A — Poursuites](../ROADMAP.md#phase-a--parité-rendu-moteur-pbr--matériaux--gltf). |
| **Shortlist Khronos** | [phase-a-khronos-shortlist.md](phase-a-khronos-shortlist.md) |
| **Checklist PBR (w3dts)** | [phase-a-pbr-checklist-w3dts.md](phase-a-pbr-checklist-w3dts.md) |
| **Fixture** | [`fixtures/phases/phase-a/`](../../fixtures/phases/phase-a/) |

## Axes prioritaires

- **Data-driven** : extensions matériau / variantes pipeline décrites par **données** (tables, JSON/RON, clés de shader) ; assets Khronos en **fixtures** versionnées.
- **Alignement exemple client (natif) et WASM** : les deux cibles s’appuient sur le **même** pipeline (`w3drs-renderer` + `pbr.wgsl`, `w3drs-assets::load_from_bytes`, mêmes `MaterialUniforms` / bind group matériau), et la **même** spec config viewer Phase A (JSON) — toute PR qui modifie l’un de ces blocs exige vérification **coordonnée** (détail : [checklist PBR, section *Alignement…*](phase-a-pbr-checklist-w3dts.md)) : `cargo check` natif + `wasm32`, rebuild `npm run build:wasm` / `www/pkg` si l’API WASM change.
- **Multithreading** : décodage / upload textures et préparation CPU en **jobs** sans bloquer le thread render ; mesurable via traces ou compteurs de frames bloquées = 0.
- **Modularité** : extensions glTF et tables de matériaux isolées dans `w3drs-assets` (ou crate dédié `w3drs-gltf-ext` si nécessaire) sans entasser la logique dans un seul module monolithique.

## Écart architecture (existant → cible)

- **Existant** : PBR + glTF **dans le code** (`w3drs-assets` / renderer) ; **KHR** du périmètre [checklist](phase-a-pbr-checklist-w3dts.md) lus + pipeline viewer **JSON** (`materials/default.json`) ; multithread decode = **encore** à structurer en jobs.
- **Cible** : extensions + variantes pilotées par **données** ; shortlist Khronos + `materials/` comme dans la [description de scène](#description-prescrite-de-la-scène-v0-rédigée-dès-maintenant) ; alignement gates w3dts.
- **Ajustement** : chaque PR indique **quel hardcode** disparaît au profit de quel fichier data ; mise à jour de [`architecture.md`](../architecture.md) section *Formats* si nouvelle règle d’import / matériau.

## Périmètre (rappel)

Extensions `KHR_materials_*`, **`KHR_texture_transform`**, pipeline matériaux versionné, stratégie shader (A1 WGSL / A2 shader graph), régression assets Khronos.

## Moyen terme — rigueur PBR / prod

Évolution **hors** du socle actuel (chargement glTF, uniforms, approximations shader documentées) — prioriser selon objectif w3dts / moteurs de réf.

- **Transmission** : *pass* opaques **derrière** la géométrie + **réfraction** (ou équivalent) et **tri / blending alpha** cohérents si l’on veut se rapprocher du rendu w3dts / moteurs de réf. (L’implémentation actuelle reste une **approximation IBL** sur un chemin monopasse, sans scène opacifiée en texture.)
- **Extensions non couvertes par `gltf` 1.4** (ex. **sheen**, **iridescence**) : **mise à niveau** de la dépendance `gltf` (si le crate l’embarque) **ou** champs JSON / extension manuelle côté chargeur — sans bloquer le périmètre Phase A courant.
- **JSON viewer (fixtures)** : **variantes** de pipeline / **RON** (ou autre) dans `fixtures/phases/phase-a/materials/` si le produit l’exige — l’[amorce JSON](phase-a-pbr-checklist-w3dts.md) (`default.json` + `phase_a_viewer_config`) s’étend dans ce sens.

*Voir aussi la [checklist PBR](phase-a-pbr-checklist-w3dts.md) (items A1/A2, WASM + natif, gates visuels).*


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

- **DOR** : [x] `fixtures/phases/phase-a/` avec reproduction documentée ([README](../../fixtures/phases/phase-a/README.md), [manifest](../../fixtures/phases/phase-a/manifest.json)) ; gate **DamagedHelmet** versionné sous `www/public/` (**Git LFS** + SHA256 dans la [shortlist](phase-a-khronos-shortlist.md)).
- **DOD** :
  - [x] Au moins un **test** `cargo test` référence explicitement `fixtures/phases/phase-a/` : [`crates/w3drs-assets/tests/phase_a_fixture.rs`](../../crates/w3drs-assets/tests/phase_a_fixture.rs) (`cargo test -p w3drs-assets --test phase_a_fixture`).
  - [x] Toute validation manuelle = **checklist** copiable dans la PR ; natif et web utilisent les **mêmes** assets lorsque les deux cibles sont dans le périmètre. **Gates** visuels (DamagedHelmet) : procédure + emargement dans [phase-a-gates-record.md](phase-a-gates-record.md) ; cases correspondantes de [phase-a-pbr-checklist-w3dts.md](phase-a-pbr-checklist-w3dts.md) *Critères visuels* cochées (**clôture 2026-04-20**).


---

## Definition of Ready (DOR)

- [x] **Shortlist d’assets** Khronos (noms + versions) : [phase-a-khronos-shortlist.md](phase-a-khronos-shortlist.md) — SHA256 du gate + pin du dépôt Khronos ; LFS documenté.
- [x] **Checklist PBR** alignée w3dts : [phase-a-pbr-checklist-w3dts.md](phase-a-pbr-checklist-w3dts.md) (**copie** des critères ; pin URL commit du dépôt w3dts **dès qu’un dépôt public** expose les gates — voir en-tête de ce fichier).
- [x] Branche de base : `cargo xtask check` **vert** (à réexécuter avant merge ; même barème que le hook pre-commit).

---

## Definition of Done (DOD)

- [x] Chaque extension / chemin matériau **du périmètre checklist** a des **tests** ciblant les branches critiques (chargeur + shader / intégration) : `cargo test -p w3drs-assets -p w3drs-renderer` + [`phase_a_fixture`](../../crates/w3drs-assets/tests/phase_a_fixture.rs) ; la barre « couverture exhaustive par extension KHR » reste une **exigence de PR** (CONTRIBUTING) pour les ajouts futurs.
- [x] **Régression** : jeu minimal d’assets Khronos (`fixtures/phases/phase-a/`, manifeste) chargé sans échec test ; captures golden **optionnelles** (non requises pour cette clôture).
- [x] **WASM + natif** : matrice de test et parité pipeline figées dans la [checklist](phase-a-pbr-checklist-w3dts.md) ; `cargo xtask check` (natif + wasm32).
- [x] **Gates** visuels w3dts (DamagedHelmet) : enregistrement [phase-a-gates-record.md](phase-a-gates-record.md) (2026-04-20) ; [critères visuels](phase-a-pbr-checklist-w3dts.md#critères-visuels--gates-damagedhelmet--scène-minimale) complétés.

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

- [x] Entrée **Phase A** dans [`../journal.md`](../journal.md) (2026-04) : PBR/IBL w3dts, exemple `hdr-ibl-skybox`, LUT 1024.
- [x] **Clôture 2026-04-20** : journal mis à jour (paragraphe clôture + lien [phase-a-gates-record.md](phase-a-gates-record.md)) ; assets = manifeste Phase A + gate DamagedHelmet (shortlist dans le ticket et la checklist).
