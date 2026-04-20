# Phase A — shortlist d’assets (régression PBR / glTF)

Liste **figée** des modèles de référence pour la parité PBR / glTF (Phase A). Les binaires volumineux suivent [`.gitattributes`](../../.gitattributes) (**Git LFS**).

## Gate principal — Khronos Sample Models

| Modèle | glTF | Chemin dans w3drs | SHA256 (octets du fichier) | Git LFS |
|--------|------|-------------------|----------------------------|---------|
| **DamagedHelmet** | 2.0 | [`www/public/damaged_helmet_source_glb.glb`](../../www/public/damaged_helmet_source_glb.glb) | `03bcd1f8b037ef2224d8fc79950d058b85fc784c0c0015f8066cde5eb87b417a` | `git lfs ls-files --long` → OID = SHA256 pour ce fichier |

**Source amont** :  
[KhronosGroup/glTF-Sample-Models — `2.0/DamagedHelmet` @ `1ba47770292486e66ca1e1161857a6e5695c2631`](https://github.com/KhronosGroup/glTF-Sample-Models/tree/1ba47770292486e66ca1e1161857a6e5695c2631/2.0/DamagedHelmet)

> Le nom de fichier w3drs (`damaged_helmet_source_glb.glb`) peut différer du amont ; la **vérité** pour CI est le **SHA256** du binaire versionné.

## Extension `KHR_materials_anisotropy` — lampe de grange (curated)

| Modèle | glTF | Chemin dans w3drs | SHA256 | Source |
|--------|------|-------------------|--------|--------|
| **AnisotropyBarnLamp** | 2.0 | [`fixtures/phases/phase-a/glb/AnisotropyBarnLamp.glb`](../../fixtures/phases/phase-a/glb/AnisotropyBarnLamp.glb) | `0e728826c30bd6f3a18a0911db9f5f9ddc2dafa8e20bf9c3312c9625d1e32a24` | [bencehari/gltf-sample-assets — `AnisotropyBarnLamp` (glTF-Binary)](https://github.com/bencehari/gltf-sample-assets/tree/main/Models/AnisotropyBarnLamp/glTF-Binary) · [fichier raw `main`](https://github.com/bencehari/gltf-sample-assets/raw/refs/heads/main/Models/AnisotropyBarnLamp/glTF-Binary/AnisotropyBarnLamp.glb) |

Empreinte calculée sur le binaire tel que vendu dans ce dépôt (réimport depuis l’URL raw si besoin de réaligner).

## Extensions encore sans GLB dédié dans le dépôt

| Extension / thème | Statut |
|-------------------|--------|
| `KHR_materials_ior` | **Lecture + F0 PBR** dans le moteur (`Material::ior`, shader) ; modèle de fixture optionnel pour QA ciblée |
| `KHR_materials_clearcoat` | idem |
| `KHR_materials_transmission` | idem (priorité produit) |

Voir le ticket [phase-A-pbr-materiaux-gltf.md](phase-A-pbr-materiaux-gltf.md) pour l’ordre d’implémentation.

---

*Dernière mise à jour des empreintes : 2026-04 (DamagedHelmet + AnisotropyBarnLamp).*
