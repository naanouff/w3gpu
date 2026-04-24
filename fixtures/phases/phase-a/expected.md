# Critères mesurables — scène Phase A v0

Critères **objectifs** pour la validation fonctionnelle (compléter la checklist PBR détaillée : [phase-a-pbr-checklist-w3dts.md](../../docs/tickets/phase-a-pbr-checklist-w3dts.md)).

## Chargement

- Le fichier dont le SHA256 est listé dans [phase-a-khronos-shortlist.md](../../docs/tickets/phase-a-khronos-shortlist.md) **parse** sans erreur fatale côté `w3drs-assets`.
- Le JSON [`materials/default.json`](materials/default.json) est **lu** par le client natif (`load_phase_a_viewer_config_or_default`) ; clés inconnues ou parse partiel : repli sur défauts + `log::warn` (pas de panic).
- Le test d’intégration [`phase_a_fixture.rs`](../../crates/w3drs-assets/tests/phase_a_fixture.rs) passe (`cargo test -p w3drs-assets --test phase_a_fixture`).
- `cargo xtask check` reste **vert** sur la branche qui intègre les changements.

## Rendu (gate DamagedHelmet)

- **Pas de fallback shader** visible sur la majorité des pixels du mesh principal (pas de rose / magenta de secours).
- **≥ 1** primitive du modèle affiche des **variations d’intensité** cohérentes avec des normales et une roughness non triviales (inspection visuelle ou capture de référence en PR).
- Sous **lumière orbitale** ou IBL active : **highlights** présents sur les parties métalliques (pas un albedo seul plat).

## Rendu (AnisotropyBarnLamp — stress `KHR_materials_anisotropy`)

- Le GLB **parse** et produit des primitives (déjà couvert par le test d’intégration).
- Lorsque l’extension est **implémentée** dans le pipeline : highlights **anisotropes** visibles sur les parties métalliques orientées (critère visuel + note en PR si écart volontaire / approximation).

## Rendu (TextureTransformTest — stress `KHR_texture_transform`)

- Le GLB **parse** et produit des primitives (test d’intégration + SHA256 dans [phase-a-khronos-shortlist.md](../../docs/tickets/phase-a-khronos-shortlist.md)).
- Les UV **ne se comportent pas comme une seule texture répétée identiquement** sur chaque panneau : offsets / rotations / échelles par `textureInfo` doivent se voir (grille Khronos : cas nominaux « Correct » vs « Error » selon le panneau).

## Rendu (MetalRoughSpheres — grille métal / rugosité + IBL)

- Le GLB **parse** et produit des primitives (test d’intégration + SHA256 dans [phase-a-khronos-shortlist.md](../../docs/tickets/phase-a-khronos-shortlist.md)).
- Les sphères métalliques montrent une **progression lisible** métal ↔ diélectrique et une variation de rugosité (pas un gris uniforme sans reflets).
- **IBL / environnement** : le client natif n’affiche **pas** de skybox géométrique séparée — seul le **clear color** remplit l’arrière-plan ; l’HDR alimente irradiance + préfiltre pour les **reflets sur les meshes**. Toute incohérence visuelle (orientation des reflets, bandes, teinte globale) se traite avec **captures** côte à côte référence / w3drs et ticket dédié.

## Suivi

- Étendre ce fichier avec des **seuils chiffrés** (SSIM, delta E, etc.) lorsque des captures *golden* seront intégrées.
