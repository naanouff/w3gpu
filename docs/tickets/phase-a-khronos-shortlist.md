# Phase A — shortlist d’assets Khronos (DOR)

Liste **figée** des modèles de référence pour la parité PBR / glTF (Phase A). Les binaires volumineux suivent [`.gitattributes`](../../.gitattributes) (**Git LFS**).

## Gate — chargé dans le dépôt aujourd’hui

| Modèle (Khronos glTF Sample Models) | glTF | Chemin dans w3drs | SHA256 (octets du fichier) | Git LFS (référence) |
|-------------------------------------|------|-------------------|----------------------------|----------------------|
| **DamagedHelmet** | 2.0 | [`www/public/damaged_helmet_source_glb.glb`](../../www/public/damaged_helmet_source_glb.glb) | `03bcd1f8b037ef2224d8fc79950d058b85fc784c0c0015f8066cde5eb87b417a` | `git lfs ls-files --long` → OID identique au SHA256 ci-dessus pour ce fichier |

**Source amont** (arborescence Khronos, commit de référence pour le dossier modèle) :  
[KhronosGroup/glTF-Sample-Models — `2.0/DamagedHelmet` @ `1ba47770292486e66ca1e1161857a6e5695c2631`](https://github.com/KhronosGroup/glTF-Sample-Models/tree/1ba47770292486e66ca1e1161857a6e5695c2631/2.0/DamagedHelmet)

> Le fichier servi par w3drs (`damaged_helmet_source_glb.glb`) peut différer par le nom du fichier glb amont (`DamagedHelmet.glb`, etc.) ; la **vérité** pour CI et régression est le **SHA256** du binaire versionné dans ce dépôt.

## Slots optionnels (extensions `KHR_materials_*`)

Non versionnés dans le dépôt au moment de la préparation DOR ; à ajouter sous `fixtures/phases/phase-a/glb/` (ou chemin manifeste) **avec SHA256** dès qu’une extension est dans le périmètre de livraison.

| Extension / thème | Exemple de modèle cible (Khronos) | Statut |
|--------------------|-----------------------------------|--------|
| `KHR_materials_anisotropy` | Exemples *Sample Models* ou *Sample Assets* Khronos portant l’extension | Non versionné |
| `KHR_materials_ior` | idem | Non versionné |
| `KHR_materials_clearcoat` | idem | Non versionné |
| `KHR_materials_transmission` | idem | Non versionné (priorité produit) |

Voir le ticket [phase-A-pbr-materiaux-gltf.md](phase-A-pbr-materiaux-gltf.md) pour l’ordre d’implémentation réel.

---

*Dernière mise à jour des empreintes : 2026-04 (gate DamagedHelmet).*
