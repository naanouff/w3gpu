# Phase C — Animation & peau

| Champ | Valeur |
|-------|--------|
| **ID** | `PHASE-C` |
| **Roadmap** | [ROADMAP § Phase C](../ROADMAP.md) |
| **Statut** | À faire |

## Axes prioritaires

- **Data-driven** : clips, courbes et tables de skinning référencés par **données** (composants / assets) ; pas d’animation hardcodée dans `main`.
- **Multithreading** : évaluation clips / préparation palette en parallèle sur plage d’entités ; tests de concurrence sur données immuables par frame.
- **Modularité** : crate ou feature `animation` distincte de `w3drs-renderer` front API.

## Écart architecture (existant → cible)

- **Existant** : pas de skinning / clips / morph **bout-en-bout** dans le loader ni le layout GPU documenté.
- **Cible** : GLB animé + palette GPU + données clips ; parité chargeur avec les plans w3dts.
- **Ajustement** : étendre `architecture.md` (*Moteur / formats*) quand le layout vertex skinned est figé.

## Périmètre

Skinning GPU, morph, clips glTF, extensions loader.


## Scène ou projet de test (validation fonctionnelle)

Chaque livraison doit inclure ou étendre **un projet de test versionné** permettant la **validation fonctionnelle** des fonctionnalités de cette phase (répétable, même sur CI dès que l’infra le permet).

| Champ | Valeur |
|-------|--------|
| **ID fixture** | phase-c |
| **Chemin cible** | fixtures/phases/phase-c/ (racine dépôt **w3drs/** ; créer au premier jalon — voir [convention](../../fixtures/phases/README.md)) |
| **Rôle** | GLB skinné + clips ; valide skinning, morph si applicable, boucle animation. |

### Évolution (ROADMAP)

| Moment | Attendu |
|--------|---------|
| **Aujourd’hui** | Dossier **scène / projet** + README.md : prérequis, commandes (cargo xtask client, www), point d’entrée (argument CLI, env, ou config). |
| **À terme** | **Workspace éditeur** + **.w3db** : chargement **natif** et **web** du **même** paquet pour QA sans divergence de données. |

### Description prescrite de la scène v0 (rédigée dès maintenant)

Spec de `fixtures/phases/phase-c/` pour skinning + clips (+ morph si périmètre).

| Élément | Contenu attendu |
|---------|-----------------|
| `README.md` | Chargement GLB ; contrôle lecture animation (pause / frame N) ; critères visuels ou probes. |
| `glb/character_skinned.glb` | GLB **minimal** avec skeleton + 1 clip (nom stable) ; SHA256. |
| `clips.json` | Liste clips, plage frames, vitesse ; données, pas de durées codées uniquement en test. |
| `expected.md` | Matrices os ou positions sommets à **t** fixe (epsilon) ; morph : deltas attendus si applicable. |

### Critères de scène (DOR / DOD)

- **DOR** : [ ] fixtures/phases/phase-c/ décrit dans le README (ou PR) avec reproduction **documentée** ; hachages / LFS pour gros binaires.
- **DOD** : [ ] au moins un **test** (cargo test / E2E) **référence** ce chemin ; toute validation manuelle = **checklist** copiable dans la PR ; natif et web utilisent les **mêmes** assets lorsque les deux cibles sont dans le périmètre.


---

## Definition of Ready (DOR)

- [ ] **GLB de référence** skinné + animation (hash, licence) dans repo ou URL miroir avec checksum.
- [ ] Layout vertex / bind group skinning **spécifié** (doc shader + Rust) avant implémentation.
- [ ] Base CI verte.

---

## Definition of Done (DOD)

- [ ] GLB référence joué en boucle **WASM et natif** ; test assert sur **bone matrix** ou pixel probe (seuil documenté).
- [ ] Tests morph si périmètre inclus : même exigence reproductible.
- [ ] Pas de fuite GPU (buffers détruits) : test ou compteur d’objets wgpu stable sur N frames.

### Outils de validation

| Outil | Rôle | Seuil |
|-------|------|--------|
| `cargo test -p w3drs-assets -p w3drs-renderer` (+ animation crate) | Parse glTF skin + runtime | 0 échec. |
| `cargo llvm-cov` | Couverture loader + runtime | Seuil PR sur diff. |
| `wasm32` check | `cargo check -p w3drs-wasm --target wasm32-unknown-unknown` | Vert. |
| Natif | `cargo xtask client` + asset référence (checklist) | Comportement identique à test d’intégration (temps à frame stable ± tolérance). |
| `miri` (optionnel) | Si `unsafe` nouveau sur buffers partagés | `cargo miri test` sur scope ciblé si adopté par le projet. |

---

## Journal

- [ ] [`../journal.md`](../journal.md) : référence GLB, schémas GPU, résultats perfs + couverture.
