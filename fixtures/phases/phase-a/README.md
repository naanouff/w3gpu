# Fixture `phase-a` — parité rendu PBR / glTF

Projet de test versionné pour le ticket [Phase A — Parité rendu moteur](../../docs/tickets/phase-A-pbr-materiaux-gltf.md).  
Shortlist Khronos et empreintes : [phase-a-khronos-shortlist.md](../../docs/tickets/phase-a-khronos-shortlist.md).  
Checklist PBR : [phase-a-pbr-checklist-w3dts.md](../../docs/tickets/phase-a-pbr-checklist-w3dts.md).

## Prérequis

- **GPU** : WebGPU (Chrome / Edge récents) pour `www/` ; adaptateur Vulkan/Metal/D3D12 pour le client natif selon plateforme.
- **Git LFS** : `git lfs pull` à la racine du dépôt pour récupérer les `.glb` sous `www/public/`.
- **Rust** stable + **wasm-pack** si vous construisez le WASM localement.

## Repro — client natif

Depuis la racine `w3drs/` :

```bash
cargo xtask client
```

Le client charge par défaut `www/public/damaged_helmet_source_glb.glb` (voir README racine). Pour pointer explicitement le même binaire que le manifeste Phase A :

```bash
cargo run -p native-triangle --release -- www/public/damaged_helmet_source_glb.glb
```

*(Sur Windows, le binaire est sous `target\release\native-triangle.exe`.)*

## Repro — web (`www/`)

```bash
cargo xtask www
```

Ouvrir `http://localhost:5173` ; le viewer charge `/damaged_helmet_source_glb.glb` (même asset que la gate Phase A).

## Tests automatisés

- `cargo test -p w3drs-assets --test phase_a_fixture` — vérifie la présence du manifeste, la **SHA256** du gate GLB (alignée sur `manifest.json`) et le parse via `w3drs_assets::load_from_bytes`.

## Fichiers de données

| Fichier | Rôle |
|---------|------|
| [`manifest.json`](manifest.json) | Liste ordonnée des GLB + empreinte attendue pour la gate courante. |
| [`glb/README.md`](glb/README.md) | Convention pour copier / vendre des binaires sous `glb/` (CI autonome). |
| [`materials/default.json`](materials/default.json) | Placeholder de paramètres / variantes **data-driven** (étendu au fil des PR). |
| [`expected.md`](expected.md) | Critères mesurables de la scène v0. |

## Checklist visuelle rapide (DamagedHelmet)

1. Casque lisible, pas de surfaces entièrement magenta / erreur shader.
2. Reflets métalliques crédibles sous lumière / IBL.
3. Normales cohérentes (détails du mesh visibles, pas « inversé » global).
4. Textures PBR apparentes (pas un gris uniforme sur tout l’objet).
5. Aucune panic au chargement ; pas d’erreurs validation layer **bloquantes** sur cet asset (si layers activées).

---

*Voir ticket Phase A pour DOD (tests automatisés référençant ce dossier).*
