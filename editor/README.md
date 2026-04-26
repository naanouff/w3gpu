# w3d-editor (coquille native)

Binaire **w3d-editor** : shell **eframe/egui** (rail 8 modes, tête, panneau inspecteur, viewport texte) ; configuration issue de [`fixtures/phases/phase-k/editor-ui.json`](../fixtures/phases/phase-k/editor-ui.json) (même spec que le shell `www/`).

## Lancer

```bash
cargo xtask editor
# ou
cargo run -p w3d-editor
```

Option : `--config CHEMIN` vers un `editor-ui.json` alternatif (UTF-8).

## Suite

- **Fait (données)** : chargement de `fixtures/phases/phase-a/materials/default.json` via `w3drs-assets` (crate `motor`) — affichage dans le panneau central ; aligné Khronos / WASM.
- **À venir (GPU)** : rendu 3D dans le panneau = **eframe** avec backend **wgpu** (pas *glow*) + *paint callback* `egui_wgpu` ; le workspace devra alors **n’avoir qu’une** ligne de version `wgpu` compatible `w3drs-renderer` (migration coordonnée, v. discussion `windows` + `wgpu` 24→29 sur DX12). Ensuite, brancher la même logique d’`encode` que `khronos-pbr-sample` sur un `TextureView` cible ou la sous-région swapchain.
- Remplacer egui par la stack cible (ex. **GPUI**) en conservant thème + layout data-driven.
