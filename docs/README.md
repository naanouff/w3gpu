# w3drs — Documentation

> Point d'entrée de la bibliothèque documentaire du projet.

## Index

| Fichier | Contenu |
|---|---|
| [Goals.md](Goals.md) | Vision produit, cibles de performance |
| [ROADMAP.md](ROADMAP.md) | Roadmap technique — alignement sur le concept **w3dts** (port Rust prod) |
| [architecture.md](architecture.md) | Hub architecture : moteur, éditeur cible, formats (`.w3db`, glTF, data), diagrammes ; détail runtime + [journal.md](journal.md) |
| [shaders.md](shaders.md) | Layout des bind groups, alignement WGSL/Rust, structs GPU |
| [journal.md](journal.md) | Journal d'implémentation — phases réalisées et décisions prises |
| [api.md](api.md) | API publique JS/WASM et Rust |
| [design/README.md](design/README.md) | Maquette éditeur natif (`Mode-based v2`) et parité `www/` allégée |
| [tickets/README.md](tickets/README.md) | Cadencement Roadmap : tickets par phase (DOR/DOD, outils, journal) |
| [fixtures/phases/README.md](../fixtures/phases/README.md) | Scènes / projets de test versionnés par phase (`fixtures/phases/phase-*`) |

## État actuel

| Phase | Description | État |
|---|---|---|
| 0 | Workspace Cargo, WASM hello world | ✅ |
| 1 | Triangle WebGPU natif + WASM | ✅ |
| 2 | ECS, PBR, glTF, textures | ✅ |
| 3 — IBL | Irradiance + prefiltered + BRDF LUT | ✅ |
| 3a | Shadow maps + Render graph + Plugin system | ✅ |
| 4 | GPU-driven : Draw Indirect + Hi-Z occlusion culling | ✅ |
| 5 | Post-processing : bloom, ACES, FXAA | ✅ |
| 3b | ECS Archetypes SoA + Rayon | ✅ |
| 6 | Éditeur multi-mode | en cours de définition — voir [ROADMAP.md](ROADMAP.md) phase K |
| 7 | SaaS bridge + Cloud compute | après parité runtime cible — voir [ROADMAP.md](ROADMAP.md) phase L |

Pour la **feuille de route détaillée** (multithreading, PBR étendu, RenderGraph, format **`.w3db`**, animation, physique, terrain, particules, audio, input, réseau, hybride raster/path trace, workspace éditeur natif), utiliser **[ROADMAP.md](ROADMAP.md)**.

## Dernières réalisations (Phase 4 → 5)

**GPU-driven pipeline (Phase 4)**
- `DrawIndexedIndirectArgs` via `entity_indirect_buf` — zéro draw call CPU pour la géométrie
- Hi-Z pyramid : `HizPass` génère une mipchain depth 64×64→1×1 chaque frame
- `CullPass` (compute) : frustum + occlusion Hi-Z → `instance_count = 0` pour les entités cachées
- Fix near-plane straddling : `any_behind` conservatif → jamais culled à tort
- Tests GPU headless (`cull_integration.rs`) : 9 cas couvrant les invariants de monotonie
- Démo native 3 scènes (Wall / Sieve / Peekaboo) avec orbit camera et métriques en titre

**Post-processing (Phase 5)**
- `HdrTarget` : rendu PBR → `RGBA16Float` (suppression Reinhard de `pbr.wgsl`)
- `bloom.wgsl` : prefilter Karis + soft-knee threshold, 9-tap gaussian séparable H/V × 2 passes
- `tonemap.wgsl` : ACES Narkowicz + bloom additif + FXAA 3×3 → sRGB swapchain
- `PostProcessPass` : chaîne complète en un `encode()`, rebuild automatique au resize
