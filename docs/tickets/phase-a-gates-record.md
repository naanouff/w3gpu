# Phase A — enregistrement des *gates* visuels (DamagedHelmet + court web)

Document **d’emargement** pour le critère de **checklist** [procédure de reprise *Critères visuels / gates*](phase-a-pbr-checklist-w3dts.md#critères-visuels--gates-damagedhelmet--scène-minimale). Remplir une ligne par **campagne** de vérification (même date pour natif + web de préférence).

Remplir **après** avoir suivi la procédure (natif + web) ; cocher enfin les cases dans [phase-a-pbr-checklist-w3dts.md](phase-a-pbr-checklist-w3dts.md) section *Critères visuels* et mettre à jour le [DOD](phase-A-pbr-materiaux-gltf.md) + [journal.md](../journal.md) si toutes les conditions sont remplies.

## Gabarit (campagnes futures)

Pour une **nouvelle** campagne après régression ou changement PBR, **ajouter une ligne** dans la table *Campagnes enregistrées* (mêmes colonnes). Sur **Windows**, préférer `npm.cmd` à `npm` si l’exécution de scripts est restreinte.

**Natif (minimum)** : depuis la racine `w3drs/`, ex. `cargo xtask client` **ou** `cargo run -p khronos-pbr-sample --release` — parcourir le [manifeste Phase A](../../fixtures/phases/phase-a/manifest.json) avec **←** / **→** ; s’attarder sur l’entrée **DamagedHelmet** (gate) puis un échantillon de la shortlist.

**Web (minimum)** : `cd www && npm run build:wasm` (si le Rust a changé), puis `npm run dev` — recharger, vérifier [viewer-manifest](../../www/public/phase-a/viewer-manifest.json) (gate DamagedHelmet) ; **Espace** = cull Hi-Z ; **←/→** si plusieurs modèles ; **aucune** erreur bloquante en console (validation WebGPU, chargement glTF, etc.).

## État

- **Dernière exécution enregistrée** : 2026-04-20 (voir ligne ci-dessous).
- **DOD visuel checklist** (cases *Critères visuels* dans la checklist) : **fermé** — cases cochées dans [phase-a-pbr-checklist-w3dts.md](phase-a-pbr-checklist-w3dts.md) ; poursuites PBR « moteur de réf » (transmission avancée, AO, A2…) restent en [Moyen terme — ticket A](phase-A-pbr-materiaux-gltf.md#moyen-terme--rigueur-pbr--prod) et [ROADMAP § Phase A — Poursuites](../ROADMAP.md#phase-a--parité-rendu-moteur-pbr--matériaux--gltf).

### Campagnes enregistrées

| Date (locale) | Natif (voir ci-dessous) | Web (voir ci-dessous) | Contrôle DamagedHelmet (OK / KO / n/a) | Autres remarques |
|---------------|-------------------------|------------------------|----------------------------------------|------------------|
| 2026-04-20 | `cargo xtask client` **ou** `cargo run -p khronos-pbr-sample --release` — manifeste [`fixtures/phases/phase-a/manifest.json`](../../fixtures/phases/phase-a/manifest.json), **←**/**→**, gate **DamagedHelmet** puis échantillon shortlist | `cargo xtask www` (sync www) ; `npm.cmd run build:wasm` si le Rust a changé ; `npm.cmd run dev` — [`viewer-manifest`](../../www/public/phase-a/viewer-manifest.json), **Espace** Hi-Z, **←**/**→**, console sans erreur bloquante | **OK** | **Visuel** : pas de magenta global sur le gate ; IBL / métal–diélectrique / textures cohérents ; stabilité (pas de panic ni validation GPU bloquante sur le gate). **Automatisé même jour** (CI locale) : `cargo test -p w3drs-assets --test phase_a_fixture`, `cargo test -p w3drs-assets -p w3drs-renderer`, `cargo xtask check` — tous verts. Alpha / transmission : tolérance alignée sur approximation monopasse (documentée checklist + ticket). |

*Création : 2026-04-24 (procédure documentée). **Clôture périmètre checklist** : 2026-04-20.*
