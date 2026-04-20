# Phase A — shortlist d’assets (régression PBR / glTF)

Liste **figée** des modèles de référence pour la parité PBR / glTF (Phase A). Les binaires volumineux suivent [`.gitattributes`](../../.gitattributes) (**Git LFS**).

Le chargeur `w3drs_assets::load_from_bytes` utilise `gltf::Gltf::from_slice_without_validation` puis `import_buffers` / `import_images`, afin que des `extensionsRequired` non reconnus par **gltf** 1.4.x (ex. `KHR_materials_clearcoat`) n’empêchent pas le chargement des fixtures listées ici.

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

## Extension `KHR_materials_clearcoat` — peinture + osier (curated)

| Modèle | glTF | Chemin dans w3drs | SHA256 | Source |
|--------|------|-------------------|--------|--------|
| **ClearCoatCarPaint** | 2.0 | [`fixtures/phases/phase-a/glb/ClearCoatCarPaint.glb`](../../fixtures/phases/phase-a/glb/ClearCoatCarPaint.glb) | `4d4b32f2ef6d341191f6b6d6834f2b192762c878cd25d44e6b4b14514cd4be93` | [raw `main`](https://github.com/bencehari/gltf-sample-assets/raw/refs/heads/main/Models/ClearCoatCarPaint/glTF-Binary/ClearCoatCarPaint.glb) · [repo](https://github.com/bencehari/gltf-sample-assets/tree/main/Models/ClearCoatCarPaint) |
| **ClearcoatWicker** | 2.0 | [`fixtures/phases/phase-a/glb/ClearcoatWicker.glb`](../../fixtures/phases/phase-a/glb/ClearcoatWicker.glb) | `f162b0cd7f8e6b7cef211eec57762165a78039676b8592ce1f965e2ddb34e843` | [raw `main`](https://github.com/bencehari/gltf-sample-assets/raw/refs/heads/main/Models/ClearcoatWicker/glTF-Binary/ClearcoatWicker.glb) · [repo](https://github.com/bencehari/gltf-sample-assets/tree/main/Models/ClearcoatWicker) |

## Extension `KHR_materials_ior` — grille de test (curated)

| Modèle | glTF | Chemin dans w3drs | SHA256 | Source |
|--------|------|-------------------|--------|--------|
| **IORTestGrid** | 2.0 | [`fixtures/phases/phase-a/glb/IORTestGrid.glb`](../../fixtures/phases/phase-a/glb/IORTestGrid.glb) | `863cf24d0e48892ec830a7c712e4eb8bf5c0fd6cc2ae2f34d213b216f0bd6c12` | [raw `main`](https://github.com/bencehari/gltf-sample-assets/raw/refs/heads/main/Models/IORTestGrid/glTF-Binary/IORTestGrid.glb) · [repo](https://github.com/bencehari/gltf-sample-assets/tree/main/Models/IORTestGrid) |

## Extensions sans GLB dédié **dans le dépôt** (hors liste ci-dessus)

| Extension / thème | Statut |
|-------------------|--------|
| `KHR_materials_transmission` | Pas de fixture vendée ici (priorité produit) |

Voir le ticket [phase-A-pbr-materiaux-gltf.md](phase-A-pbr-materiaux-gltf.md) pour l’ordre d’implémentation.

---

*Dernière mise à jour des empreintes : 2026-04 (DamagedHelmet, AnisotropyBarnLamp, ClearCoatCarPaint, ClearcoatWicker, IORTestGrid).*
