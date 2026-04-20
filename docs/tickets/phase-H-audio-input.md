# Phase H — Audio & entrée

| Champ | Valeur |
|-------|--------|
| **ID** | `PHASE-H` |
| **Roadmap** | [ROADMAP § Phase H](../ROADMAP.md) |
| **Statut** | À faire |

## Axes prioritaires

- **Data-driven** : manifeste audio, **cartes d’actions** et rebinding en **fichiers** (JSON/RON) ; aucune action critique uniquement en constantes Rust.
- **Multithreading** : décodage audio / lecture disque hors thread render ; pas de `decodeAudioData` synchrone sur le hot path sans mesure.
- **Modularité** : crates ou modules `w3drs-audio`, `w3drs-input` derrière interfaces ; pas d’appel Web Audio depuis le renderer core.

## Écart architecture (existant → cible)

- **Existant** : pas de package audio / input **data-driven** équivalent w3dts.
- **Cible** : manifeste audio + **input_map** JSON ; services branchés sur ECS / runtime.
- **Ajustement** : `architecture.md` *Formats* (JSON/RON) + contraintes Web Audio WASM.

## Périmètre

Web Audio WASM, spatialisation, input maps, gamepad / XR optionnel.


## Scène ou projet de test (validation fonctionnelle)

Chaque livraison doit inclure ou étendre **un projet de test versionné** permettant la **validation fonctionnelle** des fonctionnalités de cette phase (répétable, même sur CI dès que l’infra le permet).

| Champ | Valeur |
|-------|--------|
| **ID fixture** | phase-h |
| **Chemin cible** | fixtures/phases/phase-h/ (racine dépôt **w3drs/** ; créer au premier jalon — voir [convention](../../fixtures/phases/README.md)) |
| **Rôle** | Manifeste audio + carte d’entrée JSON ; valide mute/unmute et résolution d’actions. |

### Évolution (ROADMAP)

| Moment | Attendu |
|--------|---------|
| **Aujourd’hui** | Dossier **scène / projet** + README.md : prérequis, commandes (cargo xtask client, www), point d’entrée (argument CLI, env, ou config). |
| **À terme** | **Workspace éditeur** + **.w3db** : chargement **natif** et **web** du **même** paquet pour QA sans divergence de données. |

### Description prescrite de la scène v0 (rédigée dès maintenant)

Spec de `fixtures/phases/phase-h/` : audio + entrées.

| Élément | Contenu attendu |
|---------|-----------------|
| `README.md` | Navigateur : autoplay policy ; `resume()` ; étapes mute/unmute. |
| `audio_manifest.json` | Clips courts (WAV/OGG) + chemins + volumes ; checksums. |
| `input_map.json` | Actions, bindings clavier/souris ; séquence synthétique de test documentée. |
| `expected.md` | États `pressed` / axes attendus pour la séquence ; événements audio (play count). |

### Critères de scène (DOR / DOD)

- **DOR** : [ ] fixtures/phases/phase-h/ décrit dans le README (ou PR) avec reproduction **documentée** ; hachages / LFS pour gros binaires.
- **DOD** : [ ] au moins un **test** (cargo test / E2E) **référence** ce chemin ; toute validation manuelle = **checklist** copiable dans la PR ; natif et web utilisent les **mêmes** assets lorsque les deux cibles sont dans le périmètre.


---

## Definition of Ready (DOR)

- [ ] Fichiers **fixtures** audio (courts, licence OK) + checksums.
- [ ] Schéma JSON **InputMap** v0 documenté.
- [ ] Base CI verte.

---

## Definition of Done (DOD)

- [ ] Test **mute / unmute** reproductible : état audio mesurable (mock `AudioContext` ou stub) avec **assert** sur compteur d’appels play/stop.
- [ ] Test chargement carte d’entrée : fichier → résolution `isActionPressed` déterministe (snapshot d’états pour une séquence d’événements synthétiques).
- [ ] Scène jouable décrite par **data** uniquement (chemin fichier dans PR).

### Outils de validation

| Outil | Rôle | Seuil |
|-------|------|--------|
| `cargo test` | Mocks audio / résolution input | 0 échec. |
| Vitest (si surface TS `www/` touchée) | Même contrat côté bridge | 0 échec. |
| `thirtyfour` / `chromiumoxide` (si E2E) | Parcours mute + input dans `www/` | Scénario vert en CI headless. |
| `cargo xtask check` | wasm + native | Vert. |
| Couverture | Sur glue input/audio | Seuil diff. |

---

## Journal

- [ ] [`../journal.md`](../journal.md) : schémas, fixtures, résultats E2E si présents.
