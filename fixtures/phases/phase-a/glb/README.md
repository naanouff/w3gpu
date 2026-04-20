# Dossier `glb/` — binaires Phase A

| Fichier | Rôle |
|---------|------|
| `AnisotropyBarnLamp.glb` | Stress **`KHR_materials_anisotropy`** — source [bencehari/gltf-sample-assets](https://github.com/bencehari/gltf-sample-assets/tree/main/Models/AnisotropyBarnLamp/glTF-Binary) ; SHA256 dans [phase-a-khronos-shortlist.md](../../../docs/tickets/phase-a-khronos-shortlist.md) et [`manifest.json`](../manifest.json). |

Le **gate** historique **DamagedHelmet** reste sous `www/public/` (voir [`manifest.json`](../manifest.json) `relative_path`). Pour une CI **100 %** sous `phase-a/glb/`, copier aussi le casque ici et mettre à jour le manifeste + shortlist.

## Fournir une copie locale du casque (optionnel)

1. `git lfs pull`
2. Copier `www/public/damaged_helmet_source_glb.glb` → `glb/damaged_helmet.glb` (**même SHA256**).
3. Mettre à jour `manifest.json` → entrée `damaged_helmet_gate` : `relative_path` : `glb/damaged_helmet.glb`.

## Git LFS

Les `*.glb` sont suivis en **Git LFS** (voir `.gitattributes` à la racine). Ne pas commiter de très gros binaires **hors** LFS.
