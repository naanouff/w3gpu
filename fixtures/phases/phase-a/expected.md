# Critères mesurables — scène Phase A v0

Critères **objectifs** pour la validation fonctionnelle (compléter la checklist PBR détaillée : [phase-a-pbr-checklist-w3dts.md](../../docs/tickets/phase-a-pbr-checklist-w3dts.md)).

## Chargement

- Le fichier dont le SHA256 est listé dans [phase-a-khronos-shortlist.md](../../docs/tickets/phase-a-khronos-shortlist.md) **parse** sans erreur fatale côté `w3drs-assets`.
- Le test d’intégration [`phase_a_fixture.rs`](../../crates/w3drs-assets/tests/phase_a_fixture.rs) passe (`cargo test -p w3drs-assets --test phase_a_fixture`).
- `cargo xtask check` reste **vert** sur la branche qui intègre les changements.

## Rendu (gate DamagedHelmet)

- **Pas de fallback shader** visible sur la majorité des pixels du mesh principal (pas de rose / magenta de secours).
- **≥ 1** primitive du modèle affiche des **variations d’intensité** cohérentes avec des normales et une roughness non triviales (inspection visuelle ou capture de référence en PR).
- Sous **lumière orbitale** ou IBL active : **highlights** présents sur les parties métalliques (pas un albedo seul plat).

## Suivi

- Étendre ce fichier avec des **seuils chiffrés** (SSIM, delta E, etc.) lorsque des captures *golden* seront intégrées.
