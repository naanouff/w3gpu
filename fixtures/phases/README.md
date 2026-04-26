# Fixtures — scènes / projets de test par phase

Convention pour le **cadencement** décrit dans [`docs/tickets/README.md`](../../docs/tickets/README.md).

## Structure

Chaque phase Roadmap (A → L) dispose d’un dossier :

```text
w3drs/fixtures/phases/phase-<id>/
  README.md          # reproduction : prérequis, commandes native / web, attentes
  ...                # données : glTF, JSON graphe, workspace minimal, .w3db, etc.
```

Les identifiants `<id>` suivent les tickets (`phase-a`, `phase-b`, … `phase-l`).

| Phase | Dossier | État |
|-------|---------|------|
| A | [`phase-a/`](phase-a/) | DOR : README + manifeste + chemins documentés |
| B | [`phase-b/`](phase-b/) | Jalon v0 : [`render_graph.json`](phase-b/render_graph.json) + shaders + parseur `w3drs-render-graph` |
| K | [`phase-k/`](phase-k/) | Jalon v0 : [`editor-ui.json`](phase-k/editor-ui.json) + [`workspace/`](phase-k/workspace/) (arbre Goals) + [`extensions/hello_stub/`](phase-k/extensions/hello_stub/) — voir [Phase K](../../docs/tickets/phase-K-editeur-workspaces.md) |

## Rôle

- **Validation fonctionnelle** des fonctionnalités livrées sur le ticket.
- **Spec rédactionnelle** : le contenu attendu de chaque `phase-*` est décrit **dès maintenant** dans le ticket correspondant sous [docs/tickets/](../docs/tickets/README.md) (« Description prescrite de la scène v0 ») ; le dossier sur disque suit lors des implémentations.
- **Source unique** pour tests `cargo test`, smoke `cargo xtask client`, démo `www/`, et plus tard **E2E** (mêmes assets servis statiquement).

## Évolution

| Étape | Contenu |
|-------|---------|
| **Actuel** | Dossier + données + README ; chargement par binaires d’exemple ou tests via chemin relatif au repo. |
| **Cible** | **Workspace éditeur** + bake **`.w3db`** : le même paquet valide **natif** et **web** sans dupliquer la logique en dur dans le code. |

Les dossiers sont créés **au premier jalon** de chaque phase ; ne pas commiter de très gros binaires sans **Git LFS** (voir racine `w3drs` et `.gitattributes`).
