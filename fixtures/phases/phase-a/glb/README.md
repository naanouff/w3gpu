# Dossier `glb/` — binaires Phase A

Aujourd’hui, le **gate** principal vit sous `www/public/damaged_helmet_source_glb.glb` (LFS + SHA256 dans [phase-a-khronos-shortlist.md](../../../docs/tickets/phase-a-khronos-shortlist.md)). Le [`manifest.json`](../manifest.json) pointe vers ce chemin **relatif au dossier `phase-a/`** pour éviter une copie binaire dupliquée.

## Fournir une copie locale des binaires (CI autonome ou offline)

1. `git lfs pull`
2. Copier le fichier vers ce dossier, par exemple :  
   `glb/damaged_helmet.glb` (même contenu → **même SHA256**).
3. Mettre à jour `manifest.json` → `relative_path` : `glb/damaged_helmet.glb`.
4. Garder l’empreinte dans `docs/tickets/phase-a-khronos-shortlist.md` alignée sur les octets réels.

## Git LFS

Les `*.glb` sont suivis en **Git LFS** (voir `.gitattributes` à la racine). Ne pas commiter de très gros binaires **hors** LFS.
