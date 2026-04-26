# Phase K — Éditeur natif, workspaces, extensions

| Champ | Valeur |
|-------|--------|
| **ID** | `PHASE-K` |
| **Roadmap** | [ROADMAP § Phase K](../ROADMAP.md) |
| **Statut** | À faire |

## Ordre de priorité (produit)

- **Implémentation d’abord côté éditeur natif** (binaire desktop, intégration moteur `wgpu` / chemin `cargo xtask client` → évolution vers l’hôte auteur) : workspace, shell mode-based, thème & layout data-driven, bus UI ↔ moteur.
- **`www/`** : **même** ergonomie cible en **allégé** (second sur l’ordre de *ship* des jalons ; utile parité WASM, démo, E2E navigateur). Ne **remplace** pas le natif sur la feuille de route produit.

## Axes prioritaires

- **Data-driven** : **thème** et **layout** éditeur depuis fichiers ; workspaces comme dans [Goals.md](../Goals.md) ; bake `.w3db` scriptable.
- **Multithreading** : UI thread vs moteur : file de commandes **bornée** ; tests de pression sans deadlock.
- **Modularité** : **moteur** en crates ; **éditeur** consommateur via API stable ; extensions `register_engine(api)` isolées (dylib ou modules — choix documenté).

## Viewport 3D natif (sous-tâche technique)

Objectif : afficher le rendu moteur dans le **panneau central** de l’éditeur (`editor/`, binaire `w3d-editor`), sur la **même** logique d’`encode` que le viewer de référence.

| Sujet | Détails |
|-------|--------|
| **Alignement de versions** | Le workspace impose **`wgpu = 24`** ([`Cargo.toml`](../Cargo.toml)). Tout binaire embarquant `w3drs-renderer` *et* l’UI doit n’embarquer **qu’une** génération `wgpu` (et transitoirement une pile **`windows` / `windows_core`** cohérente côté DX12). Toute hausse (ex. pour suivre `eframe`/`egui_wgpu` récents) est un **bump monorepo**, pas seulement du crate `editor/`. |
| **Stack UI cible** | Passez **`eframe`** en backend **wgpu** (désactiver *glow* sur `w3d-editor` quand on branchera le moteur), puis *paint callback* / surface (`egui_wgpu` ou rendu cible + `egui::Image`). Référence d’intégration : exemple **custom3d** egui, et pipeline terrain **`examples/khronos-pbr-sample`** + `crates/w3drs-renderer`. |
| **État actuel** | Pas de `wgpu` dans `editor/` : raccord **données** Phase A seulement (`src/motor.rs` → `fixtures/.../phase-a/.../default.json` via `w3drs-assets`), texte d’état au centre. |

**DOR partiel (viewport)** : plan de migration `wgpu` (PR dédiée ou plan dans journal) + critère de non-régression `cargo xtask` / `khronos-pbr-sample`.

**DOD partiel (viewport)** : aire centrale = au moins une frame 3D (même scène témoin que le shortlist Khronos) sans crash ; test d’intégration minimal (headless ou golden selon l’infra).

## Écart architecture (existant → cible)

- **Existant** : **`www/`** + `khronos-pbr-sample` **sans** éditeur natif, **sans** workspace auteur, **sans** compilateur multi-cibles, plugins = **Rust statique** uniquement.
- **Cible** : éditeur **hôte** ; workspace [Goals.md](../Goals.md) ; **compilateur** (exe / Node+React / page statique) ; **plugins = DLL (natif) / wasm (web)** + manifestes — voir [`architecture.md`](../architecture.md) (*Éditeur*, *Compilateur*, *Plugins*).
- **Ajustement** : chaque jalon éditeur met à jour `architecture.md` (diagrammes UI, bus, sécurité extensions).

## Périmètre

Workspace, maquettes *mode-based* ([`docs/design/`](../design/README.md) v2 + [**v3 hi-fi** (ticket UI/UX)](phase-B-editor-ui-ux-implementation.md)), shell natif + `www/` allégé, extensions, debug overlays.


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

- [ ] Maquette(s) [`docs/design/`](../design/README.md) présente(s) dans le dépôt, dont la **v3 hi-fi** pour l’**implémentation UI/UX** détaillée dans le ticket [PHASE-B-EDITOR-UI](phase-B-editor-ui-ux-implementation.md).
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
