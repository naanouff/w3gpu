# Phase A — checklist PBR (alignement concept **w3dts**)

Ce document **satisfait le DOR** « checklist PBR alignée w3dts **copiée** ou référencée par URL commit » : la checklist est **copiée ici** sous forme de critères vérifiables. Elle synthétise les exigences de la [ROADMAP § Phase A](../ROADMAP.md) et du ticket [phase-A-pbr-materiaux-gltf.md](phase-A-pbr-materiaux-gltf.md), qui décrivent la parité visée avec le moteur de référence **w3dts** (monorepo TypeScript / WebGPU — voir ROADMAP).

## Référence w3dts (dépôt Git)

La ROADMAP indique un clone **local** voisin `w3dts/` pour les plans détaillés. **Aucun dépôt Git public** n’a été identifié au moment de la préparation pour figer une URL de commit ; dès qu’une URL stable existe, ajouter ici une ligne **Référence pin** :

- URL dépôt : `…`
- Commit : `<sha complet>`
- Fichier ou dossier de *gates* PBR : `…`

Jusqu’à ce pin, la checklist ci-dessous reste la **référence contractuelle w3drs** pour les revues Phase A.

---

## Critères fonctionnels (extensions & pipeline)

- [ ] Les extensions `KHR_materials_*` retenues pour la livraison sont **lus** depuis le glTF et **reflétées** dans le matériau GPU (pas de silence total → défaut arbitraire non documenté).
- [ ] Le pipeline matériaux reste **versionné / data-driven** là où le ticket l’exige (tables RON/JSON, pas seulement des constantes Rust dans les tests).
- [ ] **Stratégie shader** documentée pour le périmètre : branche **A1** (WGSL direct) et/ou **A2** (shader graph) — voir ticket Phase A.
- [ ] **WASM + natif** : même jeu de paramètres matériau testé, ou **matrice** dans la PR expliquant toute divergence volontaire.

## Critères visuels / gates (DamagedHelmet + scène minimale)

Pour le modèle gate ([shortlist](phase-a-khronos-shortlist.md)), en chargeant la scène de test Phase A :

- [ ] Pas de **fallback matériau** évident (ex. **magenta** / rose de secours) sur les primitives visibles du gate.
- [ ] **IBL** : reflets et éclairage cohérents (pas de sphère « morte » ou normales manifestement inversées sur tout le mesh).
- [ ] **Métal / dielectric** : zones métalliques vs isolantes lisibles (roughness perceptible).
- [ ] **Textures** : cartes base color / ORM / normales **chargées** (pas de rendu uniforme plat qui indiquerait une chaîne d’import cassée).
- [ ] **Transparence / alpha** : si le modèle expose des pixels alpha, pas de tri manifestement faux sur tout l’asset (tolérance documentée si limitation connue).
- [ ] **Stabilité** : aucune **panic** ni erreur de validation GPU **bloquante** sur l’asset gate (niveau documenté : layers de validation wgpu si applicable).

## Outils (rappel DOD — hors DOR mais utile en revue)

Voir la section *Outils de validation* du ticket Phase A (`cargo test`, `cargo xtask check`, `clippy`, etc.).

---

*Checklist rédigée pour le DOR Phase A — 2026-04.*
