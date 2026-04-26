# Workspace témoin (Phase K)

Tranche minimale de la structure cible [Goals.md](../../../../docs/Goals.md) (§ *Workspace Structure*) : même **forme d’arbre** qu’un futur projet auteur ; le contenu de `src/` est un **placeholder** jusqu’à spécification du **`.w3s`** (ou paquet) par le pipeline de build.

| Chemin | Rôle |
|--------|------|
| `assets/` | ressources brutes importées (vide au jalon) |
| `src/` | scènes / descriptions projet (JSON témoin) |
| `shaders/` | WGSL du projet |
| `dist/` | sortie bake (`.w3db`) — vide tant que l’outil n’écrit pas |
| `.w3cache/` | cache local éditeur (vide) |

L’éditeur natif ouvrira ce dossier (ou un clone) comme **racine** ; le shell `www/` continue d’importer `../editor-ui.json` au niveau `phase-k/`, pas ce sous-dossier.
