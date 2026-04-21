# Phase K — Éditeur natif, workspaces, extensions

| Champ | Valeur |
|-------|--------|
| **ID** | `PHASE-K` |
| **Roadmap** | [ROADMAP § Phase K](../ROADMAP.md) |
| **Statut** | À faire |

## Axes prioritaires

- **Data-driven** : **thème** et **layout** éditeur depuis fichiers ; workspaces comme dans [Goals.md](../Goals.md) ; bake `.w3db` scriptable.
- **Multithreading** : UI thread vs moteur : file de commandes **bornée** ; tests de pression sans deadlock.
- **Modularité** : **moteur** en crates ; **éditeur** consommateur via API stable ; extensions `register_engine(api)` isolées (dylib ou modules — choix documenté).

## Écart architecture (existant → cible)

- **Existant** : **`www/`** + `khronos-pbr-sample` **sans** éditeur natif, **sans** workspace auteur, **sans** compilateur multi-cibles, plugins = **Rust statique** uniquement.
- **Cible** : éditeur **hôte** ; workspace [Goals.md](../Goals.md) ; **compilateur** (exe / Node+React / page statique) ; **plugins = DLL (natif) / wasm (web)** + manifestes — voir [`architecture.md`](../architecture.md) (*Éditeur*, *Compilateur*, *Plugins*).
- **Ajustement** : chaque jalon éditeur met à jour `architecture.md` (diagrammes UI, bus, sécurité extensions).

## Périmètre

Workspace, maquette Mode-based v2, shell natif + `www/` allégé, extensions, debug overlays.


## Scène ou projet de test (validation fonctionnelle)

Chaque livraison doit inclure ou étendre **un projet de test versionné** permettant la **validation fonctionnelle** des fonctionnalités de cette phase (répétable, même sur CI dès que l’infra le permet).

| Champ | Valeur |
|-------|--------|
| **ID fixture** | phase-k |
| **Chemin cible** | fixtures/phases/phase-k/ (racine dépôt **w3drs/** ; créer au premier jalon — voir [convention](../../fixtures/phases/README.md)) |
| **Rôle** | Workspace éditeur exemple (arborescence Goals) + extension hello ; valide bake .w3db et reload. |

### Évolution (ROADMAP)

| Moment | Attendu |
|--------|---------|
| **Aujourd’hui** | Dossier **scène / projet** + README.md : prérequis, commandes (cargo xtask client, www), point d’entrée (argument CLI, env, ou config). |
| **À terme** | **Workspace éditeur** + **.w3db** : chargement **natif** et **web** du **même** paquet pour QA sans divergence de données. |

### Description prescrite de la scène v0 (rédigée dès maintenant)

Spec de `fixtures/phases/phase-k/` : **workspace** + extension **hello**.

| Élément | Contenu attendu |
|---------|-----------------|
| `README.md` | Ouvrir workspace ; lancer bake `.w3db` ; recharger dans runtime ; 5 étapes max. |
| `workspace/` | Arborescence [Goals.md](../Goals.md) **réduite** + 1 asset + 1 shader témoin. |
| `extensions/hello_stub/` | Extension tierce documentée (manifest + point d’entrée). |
| `expected.md` | Fichiers produits dans `dist/` ; hook extension appelé ≥ 1 fois (log ou fichier témoin). |

### Critères de scène (DOR / DOD)

- **DOR** : [ ] fixtures/phases/phase-k/ décrit dans le README (ou PR) avec reproduction **documentée** ; hachages / LFS pour gros binaires.
- **DOD** : [ ] au moins un **test** (cargo test / E2E) **référence** ce chemin ; toute validation manuelle = **checklist** copiable dans la PR ; natif et web utilisent les **mêmes** assets lorsque les deux cibles sont dans le périmètre.


---

## Definition of Ready (DOR)

- [ ] Maquette [`docs/design/`](../design/README.md) présente dans le dépôt.
- [ ] **Workspace exemple** versionné sous `examples/` ou `fixtures/editor-workspace/` avec arborescence Goals.
- [ ] API extension **draft** en `docs/` ou ADR.

---

## Definition of Done (DOD)

- [ ] Ouverture workspace exemple → édition **sans crash** ; bake → `.w3db` ; reload runtime : **test d’intégration** ou script E2E décrit (étapes numérotées + asserts sur fichiers produits).
- [ ] Extension tierce **hello** chargeable sans recompiler le cœur : test automatisé (charge + hook appelé ≥ 1 fois).
- [ ] Thème / layout : fichier modifié → rendu UI différence **mesurable** (snapshot test ou hash config chargée).

### Outils de validation

| Outil | Rôle | Seuil |
|-------|------|--------|
| `cargo test` | Workspace loader + extension mock | 0 échec. |
| UI natif | Tests snapshot ou golden (outil choisi : ex. `trycmd`, images, ou assert sur tree log) | Défini dans PR. |
| `thirtyfour` / `chromiumoxide` | Shell `www/` allégé : smoke navigation modes | Vert CI si applicable. |
| `cargo xtask check` | Cross-target | Vert. |
| Couverture | Crates éditeur / glue | Seuil diff. |

---

## Journal

- [ ] [`../journal.md`](../journal.md) : stack UI retenue, preuve extension hello, liens maquette / workspace exemple.
