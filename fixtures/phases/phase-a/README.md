# Fixture `phase-a` — parité rendu PBR / glTF

Projet de test versionné pour le ticket [Phase A — Parité rendu moteur](../../docs/tickets/phase-A-pbr-materiaux-gltf.md).  
Shortlist Khronos et empreintes : [phase-a-khronos-shortlist.md](../../docs/tickets/phase-a-khronos-shortlist.md).  
Checklist PBR : [phase-a-pbr-checklist-w3dts.md](../../docs/tickets/phase-a-pbr-checklist-w3dts.md).

## Prérequis

- **GPU** : WebGPU (Chrome / Edge récents) pour `www/` ; adaptateur Vulkan/Metal/D3D12 pour le client natif selon plateforme.
- **Git LFS** : `git lfs pull` à la racine du dépôt pour récupérer les `.glb` sous `www/public/` et sous `fixtures/phases/phase-a/glb/`.
- **Rust** stable + **wasm-pack** si vous construisez le WASM localement.

## Repro — client natif

Depuis la racine `w3drs/` :

```bash
cargo xtask client
```

`cargo xtask client` lance **`khronos-pbr-sample`**, qui enchaîne les **sept** GLB du [`manifest.json`](manifest.json) (DamagedHelmet + six entrées sous `glb/`) : **←** / **→** pour changer de modèle en boucle, **clic gauche + glisser** pour l’orbite, molette pour le zoom, IBL via `www/public/studio_small_03_2k.hdr`.

```bash
cargo run -p khronos-pbr-sample --release
```

*(Sur Windows : `target\release\khronos-pbr-sample.exe`.)*

## Repro — web (`www/`)

```bash
cargo xtask www
```

Ouvrir `http://localhost:5173` ; le viewer charge le modèle décrit par [`www/public/phase-a/viewer-manifest.json`](../../www/public/phase-a/viewer-manifest.json) (`id` = gate **DamagedHelmet** par défaut, aligné sémantiquement sur le premier enregistrement de ce [`manifest.json`](manifest.json) ; `?m=1`, **←/→** si d’autres URL sont listées). Les GLB lourds restent sous `glb/` ici : pour les servir côté Vite, copier (ou LFS) vers `www/public/…` et **ajouter** une entrée au manifeste web.

## Tests automatisés

- `cargo test -p w3drs-assets --test phase_a_fixture` — vérifie le manifeste, la **SHA256** de **chaque** entrée `models[]` dans `manifest.json`, et le parse via `w3drs_assets::load_from_bytes` (import sans validation stricte `extensionsRequired` de la crate **gltf**, pour accepter notamment `KHR_materials_clearcoat` listé comme requis par certains assets).

## Fichiers de données

| Fichier | Rôle |
|---------|------|
| [`manifest.json`](manifest.json) | Liste ordonnée des GLB + empreintes (DamagedHelmet + modèles **bencehari** sous `glb/` : anisotropie, clearcoat ×2, IOR, **MetalRoughSpheres** + **TextureTransformTest** Khronos pour `KHR_texture_transform`). |
| [`glb/README.md`](glb/README.md) | Convention pour copier / vendre des binaires sous `glb/` (CI autonome). |
| [`materials/default.json`](materials/default.json) | **Viewer** Phase A : variantes, `ibl_diffuse_scale`, bloc `tonemap` — `khronos-pbr-sample` via `load_phase_a_viewer_config_or_default` ; **WASM** : copie sous `www/public/phase-a/materials/default.json` + `parse_phase_a_viewer_config_str_or_default` / `applyPhaseAViewerConfigJson`. |
| (web seulement) [`www/public/phase-a/viewer-manifest.json`](../../www/public/phase-a/viewer-manifest.json) | Même `id` que le manifeste ici, chemins servis par Vite ; premier modèle = gate, extensions listées dès qu’un `.glb` est **copié** sous `public/`. |
| [`expected.md`](expected.md) | Critères mesurables de la scène v0. |

## Checklist visuelle rapide (DamagedHelmet)

1. Casque lisible, pas de surfaces entièrement magenta / erreur shader.
2. Reflets métalliques crédibles sous lumière / IBL.
3. Normales cohérentes (détails du mesh visibles, pas « inversé » global).
4. Textures PBR apparentes (pas un gris uniforme sur tout l’objet).
5. Aucune panic au chargement ; pas d’erreurs validation layer **bloquantes** sur cet asset (si layers activées).

## Checklist visuelle rapide (AnisotropyBarnLamp — `KHR_materials_anisotropy`)

1. Abat-jour / métal anisotrope : **direction** de l’anisotropie visible quand la caméra ou la lumière bouge (pas un highlight isotrope uniforme sur tout le métal).
2. Pas de fallback matériau (magenta) sur les parties censées être métalliques.
3. Textures et reflets cohérents (pas de mesh entièrement noir sans IBL).
4. Chargement sans panic ; extensions ignorées éventuellement **documentées** en PR jusqu’à parité shader.

## Checklist visuelle rapide (TextureTransformTest — `KHR_texture_transform`)

1. Plusieurs primitives / matériaux : les **motifs UV** (grille, flèches) ne sont pas tous identiques sur chaque quad (les `offset` / `scale` / `rotation` par texture ont un effet visible).
2. Au moins une zone affiche la texture **« Correct »** attendue pour le cas de test correspondant (pas uniquement « Error » / magenta sur tout le modèle si le pipeline est correct).
3. Chargement sans panic ; pas de repli matériau global **magenta** sur l’ensemble de la scène.

---

*Voir ticket Phase A pour DOD (tests automatisés référençant ce dossier).*
