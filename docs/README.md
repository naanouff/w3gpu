# w3gpu — Documentation

> Point d'entrée de la bibliothèque documentaire du projet.

## Index

| Fichier | Contenu |
|---|---|
| [Goals.md](Goals.md) | Vision produit, cibles de performance, roadmap long terme |
| [architecture.md](architecture.md) | Décisions architecturales, structure des crates, ECS, renderer |
| [shaders.md](shaders.md) | Layout des bind groups, alignement WGSL/Rust, structs GPU |
| [journal.md](journal.md) | Journal d'implémentation — phases réalisées et décisions prises |
| [api.md](api.md) | API publique JS/WASM et Rust |

## État actuel

| Phase | Description | État |
|---|---|---|
| 0 | Workspace Cargo, WASM hello world | ✅ |
| 1 | Triangle WebGPU natif + WASM | ✅ |
| 2 | ECS, PBR, glTF, textures | ✅ |
| 3 — IBL | Irradiance + prefiltered + BRDF LUT | ✅ |
| 3a | Shadow maps + Render graph + Plugin system | 🔜 |
| 3b | ECS Archetypes SoA | pending |
| 4 | GPU-driven (Indirect, Hi-Z) | pending |
| 5 | Post-processing | pending |
