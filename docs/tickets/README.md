# Cadencement — tickets par phase (Roadmap w3drs)

Ce dossier matérialise le **cadencement** de la [Roadmap](../ROADMAP.md) : **au minimum un fichier Markdown par phase** (A → L), avec **DOR** / **DOD** fondés sur des critères **répétables**, **reproductibles** et **mesurables** (tests, outils, seuils chiffrés).

**Port 1:1 w3dts → w3drs** : l’intention produit (équivalence **fonctionnelle** domaine par domaine, exceptions d’implémentation explicites) est définie dans le [ROADMAP — *Port 1:1*](../ROADMAP.md#port-11--définition-et-exceptions-acceptables). Chaque ticket doit rester cohérent avec cette section lorsqu’il parle d’*écart* ou de *parité*.

## Priorités transverses (toutes phases)

Toute livraison doit être évaluée sous ces trois axes (voir aussi [phase-transverses.md](phase-transverses.md)) :

1. **Data-driven** — comportement décrit par données versionnées ; le code reste interpréteur / exécuteur (voir ROADMAP, section *Tout est data-driven*).
2. **Multithreading** — travail parallèle explicite (jobs, ECS parallèle, IO hors thread render) ; profils WASM documentés.
3. **Modularité** — frontières nettes **moteur** / **éditeur** (crates, features Cargo, API stable) ; extensions sans recompiler le cœur lorsque applicable.

## Écart existant vs cible architecturale

Référence normative du **cible** : [`docs/architecture.md`](../architecture.md) (moteur, éditeur, plugins DLL/wasm, compilateur, formats). L’**existant** dans le dépôt correspond surtout au **runtime** décrit dans le [README racine](../../README.md) et dans [`journal.md`](../journal.md).

| Domaine | Existant (résumé) | Cible (résumé) |
|---------|-------------------|----------------|
| **Données / graphes** | Pipeline rendu et assets **surtout dans le code** ; peu de fichiers graphes versionnés. | **Data-driven** : render graph, shader graph, terrain, particules, scripts, physique → **fichiers** + `.w3db`. |
| **Projet / livrables** | `khronos-pbr-sample` + `www/` + chemins glTF **ad hoc**. | **Workspace** éditeur + **compilateur** (exe natif, projet Node/React, page statique). |
| **Plugins** | Trait `Plugin` **Rust** lié statiquement / workspace ; pas de chaîne DLL/wasm tierce documentée. | **Plugin = DLL/dylib/so (natif) ou wasm (web)** + manifeste ; éditeur **hôte** d’extensions. |
| **Formats** | glTF/HDR en cours ; pas de **`.w3db`**, pas d’import OBJ/STEP/splats prioritaires côté Rust. | Table des formats et priorités dans `architecture.md` ; tickets A–L pour exécution. |
| **Qualité** | Tests crates + `xtask check` ; E2E / coverage **à renforcer** (phase L). | Seuils mesurables, E2E navigateur, client natif, profils CI = fixtures. |

**Ajustement pour tous les tickets de phase** : chaque fichier `phase-*.md` inclut une section **« Écart architecture (existant → cible) »** — toute PR doit dire **quel écart** elle réduit et mettre à jour **`docs/architecture.md`** ou le ticket si la réalité a changé.

## Définitions

### DOR — *Definition of Ready*

Conditions **mesurables** avant d’engager le développement du ticket (ex. spec écrite, schéma de données validé, maquette approuvée, jeux de tests d’acceptation listés, CI verte sur la branche de base).

**Scène ou projet de test** : avant le développement, la **scène / fixture de validation** est décrite (chemin cible, contenu, hashes) — voir section homonyme dans chaque fichier de phase et la convention [`fixtures/phases/README.md`](../../fixtures/phases/README.md).

### DOD — *Definition of Done*

Conditions **mesurables** de fin (tests passent, couverture minimale sur le diff, artefacts générés, documentation à jour).

**Scène ou projet de test** : à la clôture, le dossier **`fixtures/phases/<phase-id>/`** (ou équivalent documenté) est **complet** ; au moins un test automatisé **emprunte ce chemin** ; la validation fonctionnelle nouvelle est **reproductible** depuis ce projet (voir ticket de phase).

#### Outils de validation (obligatoires dans chaque ticket)

Chaque fichier de phase inclut une sous-section **« Outils de validation »** sous la DOD : commandes exactes (`cargo …`), rapports (`llvm-cov`, rapports HTML), binaires (`xtask`, CLI bake), E2E (`thirtyfour` / `chromiumoxide` quand le périmètre touche `www/`), **tests client natif** quand le périmètre touche `examples/khronos-pbr-sample` ou le rendu natif.

Les outils doivent permettre une **revalidation** sur machine propre ou en CI sans étape manuelle non décrite.

### Journal

**Règle** : à la **clôture** d’un ticket (DOD entièrement satisfaite), **`docs/journal.md` est mis à jour** dans le même merge (ou commit immédiatement suivant) : date, résumé des livrables, liens vers PR / tickets, métriques mesurées.

## Index des phases

| Phase | Fichier | Thème |
|-------|---------|--------|
| Transverse | [phase-transverses.md](phase-transverses.md) | Data-driven, multithreading, modularité (gates) |
| A | [phase-A-pbr-materiaux-gltf.md](phase-A-pbr-materiaux-gltf.md) | Parité rendu PBR / glTF |
| B | [phase-B-graphe-rendu-compute.md](phase-B-graphe-rendu-compute.md) | Render graph + compute |
| B (éditeur) | [phase-B-editor-ui-ux-implementation.md](phase-B-editor-ui-ux-implementation.md) | **UI/UX** éditeur (maquette [Mode-based v3 hi-fi](../design/Mode-based%20v3%20hi-fi.html)) — **ID** `PHASE-B-EDITOR-UI` ; **pas** le render graph (`PHASE-B`) — aligné [Phase K](phase-K-editeur-workspaces.md) |
| C | [phase-C-animation-peau.md](phase-C-animation-peau.md) | Animation & skinning |
| D | [phase-D-format-w3db-streaming.md](phase-D-format-w3db-streaming.md) | `.w3db` & streaming |
| E | [phase-E-physique-interaction.md](phase-E-physique-interaction.md) | Physique & interaction |
| F | [phase-F-terrain-procedural.md](phase-F-terrain-procedural.md) | Terrain & procédural |
| G | [phase-G-particules-vfx.md](phase-G-particules-vfx.md) | Particules & VFX |
| H | [phase-H-audio-input.md](phase-H-audio-input.md) | Audio & input |
| I | [phase-I-rendu-hybride.md](phase-I-rendu-hybride.md) | Hybrid raster / path |
| J | [phase-J-reseau-gameplay.md](phase-J-reseau-gameplay.md) | Réseau & gameplay |
| K | [phase-K-editeur-workspaces.md](phase-K-editeur-workspaces.md) | Éditeur natif & workspaces |
| K (assistant LLM) | [phase-K-assistant-llm-ollama-integration.md](phase-K-assistant-llm-ollama-integration.md) | **LLM local** (Ollama / compatible) **→ assistant IA** éditeur (optionnel) — **ID** `PHASE-K-ASSISTANT-LLM` ; [Phase K](phase-K-editeur-workspaces.md) + [UI v3](phase-B-editor-ui-ux-implementation.md) (✦ / chat) |
| L | [phase-L-industrialisation.md](phase-L-industrialisation.md) | Prod, CI, sécurité, livraisons |

*Deux entrées **B*** : le ticket **render graph** ([`PHASE-B`](phase-B-graphe-rendu-compute.md)) relève de la [Roadmap § B](../ROADMAP.md#phase-b--graphe-de-rendu--compute-équivalent-rendergraph-w3dts). Le ticket **[éditeur UI/UX](phase-B-editor-ui-ux-implementation.md)** (ID `PHASE-B-EDITOR-UI`) décrit l’implémentation UI/UX de la maquette w3d **v3 hi-fi** et se rattache surtout à la [Phase K](../ROADMAP.md) (*Éditeur natif, workspaces…*).

*Deux entrées **K*** : le ticket **[workspaces & extensions](phase-K-editeur-workspaces.md)** (ID `PHASE-K`) couvre l’éditeur hôte ; le ticket **[assistant LLM Ollama](phase-K-assistant-llm-ollama-integration.md)** (ID `PHASE-K-ASSISTANT-LLM`) vise l’**intégration d’un LLM local** pour alimenter l’**assistant IA** (sidecar HTTP, `assistant.json`, V1/V2), **sans** dépendre du moteur PBR/ECS.

## Scène ou projet de test (obligatoire par ticket / phase)

Chaque ticket de phase porte une **scène ou un projet de test** versionné, unique objectif : **valider fonctionnellement** ce qui est implémenté (régression incluse), de façon **répétable** et **mesurable**.

| Étape | Forme | Rôle |
|-------|--------|------|
| **Court terme** | Dossier sous [`fixtures/phases/<id>/`](../../fixtures/phases/README.md) + `README.md` | Données + instructions pour **`khronos-pbr-sample`** et/ou **`www/`** (selon périmètre). |
| **À terme** | **Workspace éditeur** + export **`.w3db`** | Même vérité **natif + web** : chargement du paquet binaire pour QA et E2E sans dupliquer la scène en code. |

Les tests automatisés et les outils listés en **DOD** doivent **référencer explicitement** ce chemin (pas de scène « seulement dans la tête du dev »).

La **description détaillée** du contenu (fichiers, critères `expected.md`, etc.) est rédigée **dès maintenant** dans chaque ticket de phase, section **« Description prescrite de la scène v0 »** — le dossier `fixtures/phases/<id>/` matérialise cette spec au fil des PR.

## Workflow proposé

1. Créer ou étendre **`fixtures/phases/<phase-id>/`** (voir ticket) + README avant la feature reviewable.
2. Ouvrir / dériver des sous-tickets (issues) à partir du fichier de phase.
3. Vérifier le **DOR** avant développement (dont scène de test).
4. Implémenter ; faire passer la **DOD** (outils listés + scène de test référencée par les tests).
5. Mettre à jour **`journal.md`** + **barre de progression** (ROADMAP) si jalon significatif.

---

*Création cadencement : 2026-04.*
