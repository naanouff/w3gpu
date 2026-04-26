# Roadmap w3drs — alignement sur w3dts (version Rust « prod »)

> **Objectif** : porter le **monorepo de référence w3dts** (moteur WebGPU, packages autour) vers **w3drs** de façon **1:1** au sens **produit** : chaque **domaine** que w3dts couvre (moteur, chargement, graphes, animation, outillage, plateforme, etc.) doit trouver un **équivalent w3drs** avec le **même comportement observable** (mêmes scènes de test / gates de validation possibles), l’implémentation étant en **Rust** (WASM + natif), **multithread** où c’est permis, et **data-driven** par défaut.

> Ce qu’un port **1:1** n’exige **pas** : recopie **ligne à ligne** du TypeScript, ni le même **framework UI** (React) pour l’éditeur. Ce qu’il exige : **équivalence fonctionnelle** domaine par domaine ; les exceptions d’**implémentation** (fichier sur disque, stack UI) restent explicites ci-dessous pour éviter toute ambiguïté avec w3dts.

**Référence** : périmètre et jalons w3dts documentés dans le dépôt voisin `w3dts/` (readme, `work in progress/roadmap.md`, plans par domaine : hybrid renderer, audio, input, animation, GPROC, etc.).

## Port 1:1 — définition et exceptions acceptables

Cette section **fige l’intention produit** ; les tickets de phase (A–L) et le [README tickets](tickets/README.md) s’y alignent.

| Sujet | Règle 1:1 | Exception ou traduction w3drs |
|-------|-----------|----------------------------------|
| **Moteur runtime** (PBR, ECS, render graph, loaders, etc.) | Comportement aligné w3dts sur les mêmes jeux d’essai / checklists (ex. [Phase A checklist](tickets/phase-a-pbr-checklist-w3dts.md)) | API Rust / `wgpu` / ECS SoA : non comparables en syntaxe, **oui** en résultat |
| **Format projet / scène packagée** | Même *rôle* : un artefact de projet prêt exécution, streaming, validation | **`.w3db`** (binaire versionné) remplace **SceneDD** / **scenepak** *on-wire* : **équivalent de rôle** (1:1 fonctionnel, pas 1:1 octets) — voir [Phase D](#phase-d--format-w3db--streaming-remplace-scenedd--scenepak) |
| **Viewer / éditeur** | Même *flux* auteur : ouvrir projet, éditer, lancer, déboguer, extensions | **Éditeur natif** + **workspace** + shell **`www/`** allégé (pas *clone* de l’UI React) — exigence de **même couverture de tâches** cible, pas *mêmes écrans pixel pour pixel* — [Phase K](#phase-k--éditeur-natif-workspaces-extensions--dx-développeur) |
| **Multithreading** | w3drs doit exploiter le parallélisme (ECS, jobs) là où w3dts restait souvent **mono-thread** JS | Considéré comme **dépassement** (axe *bonus* dans la barre de progression), **pas** comme entorse au 1:1 fonctionnel |
| **Périmètre** | Par défaut : l’**ensemble** des plans / packages w3dts pertinents pour le produit, sauf renoncement explicite ailleurs (README, priorisation) | Découpage par **phases A–L** (sections ci-dessous) : clôture **mesurable** (DOD) par domaine |

En cas de conflit entre une note ancienne et cette section, **cette section prévaut** pour l’**intention** ; les phases restent la feuille de route d’**exécution**.

---

## Principes d’architecture produit (w3drs)

### Multithreading au centre

**w3drs est axé autour du multithreading** : le modèle d’exécution par défaut doit exploiter plusieurs cœurs de façon **sûre** et **prévisible** (ECS + parallélisation type Rayon sur les systèmes data-oriented, tâches de chargement / décompression / cook en arrière-plan, séparation stricte des accès au device GPU). Toute nouvelle brique (animation, physique, streaming, audio decode, réseau) doit être conçue avec **jobs** ou **queues** explicites, pas comme du code « mono-thread par défaut » raccroché au thread de rendu.

Sur **WASM**, les contraintes navigateur (pas de threads lourds partout, pas de SharedArrayBuffer selon contexte) imposent des **profils** documentés ; le **natif** doit montrer la cible de parallélisme maximal.

### Format `.w3db` (remplace SceneDD / scenepak)

Le contenu projet packagé pour le runtime w3drs est le **format binaire `.w3db`** : un artefact **unique** (ou famille versionnée) qui **encapsule les données d’un projet** prêtes à l’exécution — scènes, ressources référencées, tables LOD / streaming, métadonnées de build, etc. Les formats **SceneDD** et **scenepak** du monde w3dts servent de **référence fonctionnelle** (checklists capacités), pas de format cible sur disque pour w3drs.

- **Spécification** : schéma binaire versionné, migrations, validation stricte à l’ouverture.
- **Pipeline** : import depuis sources éditeur (voir workspace) → bake → `.w3db` ; chargement runtime **mmap / stream** quand pertinent.

### Éditeur natif : projets = **workspaces**

Pour l’**éditeur natif** w3drs, un **projet** est toujours manipulé comme un **workspace** sur disque — répertoire racine avec la structure et le flux de travail décrits dans [Goals.md](Goals.md) (dossiers `assets/`, `src/`, `shaders/`, sortie `dist/` vers `.w3db`, cache local, etc.). L’éditeur ouvre et synchronise ce workspace ; le `.w3db` est le **livrable** ou la **vue packagée** du projet, pas un substitut au modèle fichier + dossiers pendant la création.

### Tout est **data-driven**

**Règle d’or** : la configuration et le comportement **visibles ou modifiables par le contenu / l’auteur** ne sont **pas** codés en dur dans le Rust (ni ailleurs de façon non sérialisable) : ils vivent dans des **données versionnées** (fichiers, blobs dans `.w3db`, schémas documentés) chargées et validées au runtime.

Le code du moteur et de l’éditeur se limite à des **moteurs génériques** : lecture de données, validation, exécution (graphes, VM, solvers), accès GPU — **pas** de définitions ad hoc de scènes, pipelines, matériaux, scripts, mondes physiques, terrains, particules, thèmes ou layouts d’UI « uniquement dans le source ».

**Périmètre typique (liste non exhaustive)** :

| Domaine | Exprimé en données (exemples) |
|--------|-------------------------------|
| **Configuration moteur** | Profils, capacités device, toggles features, chemins par défaut |
| **Render graph** | Description déclarative des passes, ressources, liens (JSON/RON/binaire selon spec) |
| **Shader graph** | Graphe de nœuds + métadonnées de compilation → WGSL |
| **Scripting** | bytecode / AST / scripts logiques référencés par le workspace ou `.w3db` |
| **Physique** | mondes, matériaux, contraintes, couches collision |
| **Particle graph** | émetteurs, modules simulation, intégration render |
| **Terrain** | heightfields, règles LOD, règles de scatter / biomes |
| **Éditeur** | **thème**, **layout** (docking, panneaux), raccourcis, presets UI |

**Formulation stricte** : **rien** de ce périmètre ne doit être **défini explicitement dans le code** comme seule source de vérité ; toute exception doit être **justifiée, documentée et minimale** (ex. invariant de sûreté non sérialisable, limite matérielle absolue) et passée en revue comme telle.

---

## Principe de priorisation

0. **Multithreading** : chaque phase ci-dessous doit préciser où le travail parallèle vit (jobs, pools, accès GPU) — pas de régression vers un cœur unique implicite.

0bis. **Data-driven** : toute nouvelle brique livrable par un projet ou l’éditeur s’exprime d’abord en **schéma + données** ; le code n’ajoute que l’interpréteur / l’exécuteur.

1. **Noyau rendu + données** d’abord (parité visuelle et chargement de scène / `.w3db`).
2. **Simulation & monde** (physique, terrain, particules) ensuite.
3. **Auteur & outillage** (éditeur, graphes, debug) en parallèle ou juste après selon besoin produit.
4. **Plateforme** (audio, input avancé, réseau, XR) pour fermer une boucle « jeu / app » complète.

Chaque grande brique doit viser : **API stable**, **tests** (unitaires + intégration GPU quand c’est possible), **docs** et **critères de done** mesurables (assets Khronos, scènes de référence, benchmarks ECS).

---

## État livré aujourd’hui (baseline w3drs)

Référence : [README.md](../README.md), [journal.md](journal.md).

| Domaine | w3drs (état) | Écart vs concept w3dts |
|--------|----------------|-------------------------|
| ECS SoA + scheduling | ✅ Solide (archetypes, Rayon) | w3dts : ECS dynamique TS — **parité conceptuelle** à maintenir côté ergonomie API |
| Rendu PBR + IBL + ombres + post | ✅ (PBR/IBL **alignés w3dts** 2026-04) | Reste : `KHR_specular`, transmission, AO, **Shader Graph** / matériaux 100 % data-driven |
| Pipeline GPU-driven (indirect, Hi-Z) | ✅ | w3dts : RenderGraph déclaratif JSON + compute passes génériques — **écart d’architecture** |
| Loader glTF / textures de base | ✅ Partiel | w3dts : loader riche + matériaux étendus + skinning/morph — **écart majeur** |
| Format projet / streaming (`.w3db`) | ❌ | Remplace **SceneDD / scenepak** côté w3drs — **binaire** encapsulant les données projet ; spec + runtime à produire |
| Physique, terrain, particules, audio, input package, P2P | ❌ | Packages w3dts à **porter ou intégrer** (ex. libs natives + pont WASM) |
| Éditeur / viewer | 🔜 (README phase 6) | w3dts : viewer-editor React — w3drs : **éditeur natif** centré **workspace** (voir [Goals.md](Goals.md)) + runtime WASM pour intégration web si besoin |

---

## Phase A — Parité rendu « moteur » (PBR + matériaux + glTF)

**But** : égaler le **niveau de fidélité** des assets PBR que w3dts vise avec ses gates de validation (ex. intégration glTF → matériau → pipeline).

**État (2026-04)** : le fragment PBR et l’IBL sont **alignés sur w3dts** (`pbr_functions` + `pbr_master_node` : GGX + `GeometrySmith4` en direct, TBN / anisotropie, IBL avec bent normal + `MAX_REFLECTION_LOD = 4`, LUT BRDF 1024 échantillons). Voir [`docs/journal.md`](journal.md) (Phase A). Exemple **`hdr-ibl-skybox`** pour debug IBL / HDR.

**Exemples client** : `khronos-pbr-sample` (natif) et `w3drs-wasm` + `www/` (WASM) partagent le **même** moteur de rendu et la spec **JSON** viewer Phase A — barème de revue et parité *démo* vs *moteur* : [checklist PBR, *Alignement…*](tickets/phase-a-pbr-checklist-w3dts.md).

- [x] **Noyau PBR + IBL** : parité formules w3dts sur le chemin **natif + WASM** ; `FrameUniforms` (flags IBL, scale diffuse).
- [x] **Lecture** `KHR_materials_*` ciblés (p.ex. *specular*, *transmission*, *volume*, *emissive strength*) : données → GPU ; la **transmission** reste une **approximation** monopasse (IBL) jusqu’au graphe *moyen terme* (voir [ticket Phase A — *Moyen terme*](tickets/phase-A-pbr-materiaux-gltf.md)).
- [ ] **Poursuites** : AO texture, autres KHR, ou **finition** transmission (opaque derrière, réfraction, tri alpha) — idem ticket **Moyen terme** ; sheen / iridescence si `gltf` + pipeline — ticket.
- [ ] **Matériaux** : pipeline **versionné / data-driven** (tables RON/JSON, clés de variante) au-delà de `default.json` — [phase-A-pbr-materiaux-gltf.md](tickets/phase-A-pbr-materiaux-gltf.md).
- [ ] **Shader authoring** : **A1** (WGSL + bibliothèque partagée) **en production** pour l’instant ; **A2** (shader graph) — spec + prototype si parité w3dts requise.
- [x] **Tests de régression** (périmètre Phase A checklist) : shortlist Khronos via fixture + tests `phase_a_fixture` / crates assets & renderer ; **checklist visuelle** fermée — [phase-a-gates-record.md](tickets/phase-a-gates-record.md) (2026-04-20). Captures golden **optionnelles** (SSIM) si CI GPU : toujours hors obligation.

**Critère de sortie** : une shortlist d’assets glTF Sample Models rendus **sans artefact bloquant** sur la même checklist que w3dts pour le périmètre PBR retenu — **atteint** pour le périmètre figé (checklist + gates) ; voir **Poursuites** ci-dessus pour l’écart restant vs w3dts « plein prod ».

---

## Phase B — Graphe de rendu & compute (équivalent RenderGraph w3dts)

**But** : même **flexibilité** que le RenderGraph w3dts : passes raster + compute, ressources buffers/textures, dispatch direct/indirect, reconfiguration contrôlée.

**Jalons détaillés** (B.1–B.7) : [ticket Phase B](tickets/phase-B-graphe-rendu-compute.md#plan-dexécution--exécuteur-complet--wasm-cible-w3dts) — **B.1–B.7** en **v0** (B.6 labels ECS + hôte, B.7 `raster_depth_mesh` + hôte) ; [Phase B](tickets/phase-B-graphe-rendu-compute.md#poursuites-hors-périmètre-v0) = barrières wgpu explicites, câblage moteur↔JSON (HDR, etc.) *ultérieur*.

- [x] **Description déclarative** + exécuteur `wgpu` v0 (natif + **WASM** `w3drsPhaseBGraphRunChecksum`) + parse/validation + fixture + tests.
- [x] **Registre** `RenderGraphGpuRegistry` (buffers/textures nommés, resize, mips) — périmètre v0.
- [x] **Exécuteurs** : `compute` (g0/g1, indirect), `raster_mesh`, `fullscreen`, `blit`, `raster_depth_mesh` (B.7) — v0.
- [x] **B.6** : `ecs_before` / `ecs_after` + `RenderGraphV0Host::ecs_node` (natif ; WASM no-op si hôte par défaut).
- [x] **Viewer** : `khronos-pbr-sample` — `--render-graph` + slot ; même `CommandEncoder` que le moteur.
- [ ] **Poursuite** : barrières wgpu explicites hors pass ; remplacer des passes moteur (HDR, shadow) par câblage ressource↔`insert_buffer` / `insert_texture_2d` + même JSON.

**Critère de sortie (v0 atteint)** : démo « compute + raster » pilotée par le JSON + shaders **sans fork** moteur ; **même** graphe validé / exécuté **natif + web** (checksum reproductible).

---

## Phase C — Animation & peau (package animation w3dts)

- [ ] **Skinning** : joints, poids, palette GPU, passes dédiées ou intégration au batching existant.
- [ ] **Morph targets** : deltas, blending, limits WebGPU.
- [ ] **Clips** : échantillonnage TRS / joints, blending, events optionnels.
- [ ] **Loader** : extension du chargeur glTF pour les champs d’animation déjà décrits dans les plans w3dts.

**Critère de sortie** : au moins un GLB skinné + une animation glTF jouée en boucle sur WASM et natif.

---

## Phase D — Format `.w3db` & streaming (remplace SceneDD / scenepak)

**But** : définir et implémenter le **format binaire `.w3db`** qui **encapsule les données d’un projet** pour le runtime — équivalent *fonctionnel* des scènes packagées / streaming w3dts, **sans** reprendre les formats SceneDD ou scenepak sur le fil.

- [ ] **Spécification binaire** : entités, composants, blobs ressources, index LOD / streaming, manifeste, extensions (audio, terrain, …) — les plans w3dts (`SCENEDD_EVOLUTION_PLAN.md`, etc.) restent une **checklist de capacités**, pas un format on-wire pour w3drs.
- [ ] **Runtime** : ouverture `.w3db`, chargement incrémental, priorités, annulation ; lecture compatible **multithread** (IO + decode en jobs, sync vers frame main).
- [ ] **Outils CLI** : bake workspace → `.w3db`, validate, diff de versions ; import depuis glTF + ressources annexes.

**Critère de sortie** : chargement d’une scène « moyenne » depuis un **`.w3db`** avec **TTFP** et mémoire bornés (métriques documentées), sans bloquer le thread render sur l’IO disque.

---

## Phase E — Physique & interaction

- [ ] **Couche physique** : intégration d’un moteur mature (ex. Rapier) côté natif ; stratégie WASM (simd, threads selon navigateur) documentée.
- [ ] **Collision / triggers** : composants ECS, événements vers gameplay.
- [ ] **Navigation** : navmesh ou équivalent (alignement futur avec `@naanouff/w3dts-navmesh`).

**Critère de sortie** : scène démo stable (pile d’objets, personnage contrôlé) 60 FPS sur une machine de référence.

---

## Phase F — Terrain & géométrie procédurale (terrain + GPROC)

- [ ] **Terrain** : LOD, heightfield ou clipmaps — voir notes UltraTerrain w3dts comme exigences fonctionnelles.
- [ ] **GPROC équivalent** : graphes de géométrie CPU (SoA), exécuteur topologique, bibliothèque de nœuds MVP (primitives, merge, instances).

**Critère de sortie** : terrain visible à l’infini + petite scène procédurale **data-driven** (sans recompile).

---

## Phase G — Particules & effets avancés

- [ ] Simulation **compute** (buffers structurés, indirect draw).
- [ ] **Tri / culling** particules cohérent avec Hi-Z existant si pertinent.
- [ ] Courbes d’émission, collision simple, attachement à l’ECS.

**Critère de sortie** : N particules (cible numérique à fixer) avec coût CPU quasi constant côté game thread.

---

## Phase H — Audio & entrée (packages w3dts-audio / w3dts-input)

- [ ] **Audio** : sur WASM, Web Audio via bindings ; spatialisation liée à la caméra ; préchargement depuis le manifeste de scène.
- [ ] **Input** : cartes d’actions, rebinding sérialisable, souris/clavier ; gamepad puis **XR** en option (priorité basse tant que WebXR n’est pas requis par le produit).

**Critère de sortie** : même scène jouable en **mute / unmute**, avec schéma d’entrée chargé depuis fichier.

---

## Phase I — Rendu hybride & qualité « offline-like » (optionnel mais dans le concept w3dts)

Référence w3dts : `HYBRID_RASTER_PATHTRACE_PLAN.md`.

- [ ] **Path tracing** ou denoising (OIDN côté natif seulement si contrainte légale/technique ; autre stratégie sur WASM).
- [ ] **Commutation** raster / path pour l’éditeur ou captures HQ.

**Critère de sortie** : une image de référence produite par le chemin hybride, comparable à une capture w3dts sur la même scène simplifiée.

---

## Phase J — Réseau & gameplay modules (P2P, combat, character, IA)

- [ ] **Multiplayer** : modèle P2P + relay (équivalent scripts w3dts) ; abstraction transport sans bloquer le thread render.
- [ ] **Gameplay** : modules « thin » (combat, character) — surtout **patterns ECS** et exemples ; pas nécessaire de tout porter si hors scope produit.

**Critère de sortie** : démo **2 clients** synchronisés sur une action simple (spawn / déplacement).

---

## Phase K — Éditeur natif, workspaces, extensions, DX développeur

**Priorité d’implémentation (produit) :** les **jalons shell / workspace / thème** sont livrés **d’abord sur l’éditeur natif** (client desktop, stack UI à trancher — cible visuelle : dense / premium *dev-tool* type **GPUI·Zed**, warm gray + **ambre** *Rust* ; maquette : [v3 hi-fi `docs/design/`](design/README.md)). Le shell **`www/`** reste la **surface allégée** de la même ergonomie (parité de flux) et le chemin **WASM** pour démo / CI, **sans précéder** le natif sur l’ordre de livraison des écrans.

- [ ] **Modèle projet** : l’éditeur natif travaille sur un **workspace** (répertoire racine du projet, aligné sur la structure décrite dans [Goals.md](Goals.md)) : assets sources, scènes logiques, shaders, caches, sortie `dist/*.w3db` — pas uniquement « un fichier ouvert » sans contexte de dossier.
- [ ] **Référence UX** : maquette *mode-based* `Mode-based v2.html` (w3gpu editor) — copie versionnée sous [`docs/design/`](design/README.md) ; le dossier **`www/`** cible une **version allégée** de la même ergonomie (modes, flux) pour le shell web.
- [ ] **Shell d’édition** : UI native (stack à trancher : ex. winit + kit UI ou Tauri) branchée sur les crates moteur ; shell **`www/`** allégé en parallèle pour démo / édition légère dans le navigateur.
- [ ] **Extensions** : contrat `register_engine(api)` / plugins dynamiques — aligné sur la vision « extensions runtime » w3dts ; chargement d’extensions compatible avec le **cycle de vie multithread** du moteur.
- [ ] **Debug** : overlay perf, inspecteur ECS, capture GPU (niveau `@naanouff/w3dts-gpudebug`).

**Critère de sortie** : ouverture d’un workspace exemple, édition, bake vers `.w3db`, relecture par le runtime ; extension tierce chargée sans recompiler le cœur.

---

## Phase L — Industrialisation « prod »

- [ ] **CI** : `cargo fmt`, `clippy -D warnings`, tests sur `wasm32-unknown-unknown`, **couverture** Rust/TS sur le code livré, suite **E2E** navigateur (harnais type **thirtyfour** / **chromiumoxide**, voir [CONTRIBUTING.md](../CONTRIBUTING.md)), **tests fonctionnels client natif** (`examples/khronos-pbr-sample` / intégration `wgpu` natif, voir même section), benchmarks non régressifs.
- [ ] **Stabilité API** : semver pour crates + politique de dépréciation pour `wasm-bindgen`.
- [ ] **Sécurité & supply chain** : `cargo deny` / advisories, politique de dépendances.
- [ ] **Livraisons** : changelog, artefacts npm pour le paquet WASM, builds natifs signés si besoin.

---

## Synthèse : remapping des anciennes phases README

| Ancienne entrée README | Nouveau positionnement |
|------------------------|-------------------------|
| Phases 0–5 (déjà ✅) | **Fondations** — conservées ; continuent d’être la base des phases A–B |
| Phase 6 — Éditeur | Raccourci vers **Phase K** (outillage + extensions) |
| Phase 7 — SaaS / cloud | **Phase L** + extensions futures (compilation shader cloud, bake GI) — **après** parité runtime w3dts sur le périmètre choisi |

---

## Comment utiliser ce document

- Pour chaque **phase**, ouvrir le document w3dts correspondant (s’il existe) et en extraire une **checklist de capacités**, puis la traduire en **tâches Rust** (crate, API, tests).
- Utiliser le cadencement **[tickets/](tickets/README.md)** : un fichier par phase (minimum) avec **DOR** / **DOD** mesurables, **scène ou projet de test** sous [`fixtures/phases/`](../fixtures/phases/README.md), et **outils de validation** listés ; à la **clôture** d’un ticket, mettre à jour [journal.md](journal.md).
- Mettre à jour la section **Barre de progression** ci-dessous lors des jalons (release, revue de parité).

---

## Barre de progression — w3drs **vs** état actuel **w3dts**

Cette section est une **vue documentaire** (pour humains et agents), pas une mesure CI. Les pourcentages sont des **estimations de couverture fonctionnelle** : *« à quel point w3drs offre aujourd’hui ce que w3dts couvre déjà dans son monorepo TypeScript ? »* — puis un **axe bonus** pour ce que w3drs vise **au-delà** de w3dts (d’où un **total pouvant dépasser 100 %** une fois la parité atteinte).

### Lecture des indicateurs

| Indicateur | Échelle | Sens |
|------------|---------|------|
| **Parité w3dts** | **0 → 100 %** | Couverture du **périmètre fonctionnel** de référence w3dts (packages viewer, core, loaders, graphes, etc.) — **100 %** = équivalence intentionnelle sur les domaines retenus pour le port, pas ligne de code TS ↔ Rust. |
| **Bonus w3drs** | **0 → ~50 %** (plafond indicatif) | Capacités **hors ou au-delà** du modèle w3dts tel qu’exécuté aujourd’hui (ex. **multithreading** natif déterministe, **client desktop** first-class, **`.w3db`**, industrialisation data-driven stricte, perf GPU-driven poussée, etc.). |
| **Indice total** | **Somme** Parité + Bonus | Peut **dépasser 100 %** : ex. **100 %** de parité + **40 %** de bonus ⇒ **140 %** sur l’échelle illustrée ci-dessous. |

**Barres globales** *(snapshot à maintenir ; chiffres indicatifs **2026-04**)* :

```
Parité w3dts    [█████░░░░░░░░░░░░░░░]  ~38 %
Bonus w3drs     [████░░░░░░░░░░░░░░░░]  ~22 %
─────────────────────────────────────────────────
Indice total    [███████░░░░░░░░░░░░░]  ~60 %  sur ciel max illustré ~150 % (100 % parité + 50 % bonus)
```

*(Représentation : chaque « █ » ≈ 5 points sur l’échelle du segment ; ajuster les compteurs à chaque revue — révision 2026-04-20, Phase A PBR ; **2026-04-24** : section *Port 1:1* + tickets / gates / plan Phase B documentés — **pas** de recalcul chiffré de parité sur cette seule mise à jour doc.)*

### Détail par domaine *(parité seule, 0–100 % par ligne)*

| Domaine | Parité w3drs → w3dts *(estim.)* | Commentaire rapide |
|---------|----------------------------------|----------------------|
| Rendu PBR + IBL + ombres + post | ~88 % | PBR/IBL calqués w3dts (2026-04) ; reste : specular ext., transmission, AO, gates checklist. |
| glTF / matériaux avancés / shader graph | ~28 % | Extensions lues (aniso, IOR, clearcoat, texture_transform) ; data-driven + specular/transmission + shader graph à cadrer. |
| Render graph déclaratif + compute | ~30 % | w3dts : JSON + exécuteurs ; w3drs : pipeline puissant mais moins « data-only ». |
| ECS & scheduling | ~70 % | Modèles différents ; force w3drs côté perf / SoA. |
| Animation / skinning / morph | ~10 % | Chantier majeur des deux côtés ; alignement à suivre. |
| Scène / streaming / format pack | ~0 % | w3dts : SceneDD / scenepak ; w3drs : **`.w3db`** à spécifier — parité comptée au chargement data équivalent. |
| Physique | ~0 % | Package w3dts ; absent côté w3drs. |
| Terrain / procédural | ~0 % | Idem. |
| Particules & VFX data-driven | ~15 % | Compute + rendu w3drs ; graphe particules type w3dts non repris. |
| Audio / input avancés | ~0 % | Plans w3dts ; runtime w3drs minimal. |
| Réseau P2P / gameplay | ~0 % | Packages w3dts ; non portés. |
| Hybrid raster / path | ~0 % | Plan w3dts ; non présent w3drs. |
| Viewer / éditeur / extensions | ~20 % | w3dts : viewer-editor React mature ; w3drs : `www/` + `khronos-pbr-sample`, maquette éditeur. |

**Moyenne indicative** des lignes ci-dessus ≈ **22 %** ; la **parité globale ~32 %** en tête de section intègre un **poids** un peu plus fort sur le rendu core (déjà avancé). Recalculer explicitement (feuille ou script) quand la politique de poids sera figée.

### Pistes de bonus *(contribuent au « > 100 % »)*

Exemples de postes **bonus** (déjà partiellement couverts ou cibles w3drs) :

- **Multithreading** productif (ECS + jobs + natif) — peu ou pas équivalent dans le thread principal JS de w3dts.
- **Client natif** wgpu (Vulkan / Metal / DX12) au même titre que WASM.
- **`.w3db`** + pipeline bake / prod.
- **Data-driven** strict (ROADMAP) sur render graph, shaders, UI éditeur, etc.
- **Tests** / CI / couverture imposés (CONTRIBUTING) au niveau « moteur pro ».

---

Dernière révision : **2026-04-24** — *Port 1:1* (définition + exceptions) intégré en tête de document ; [journal.md](journal.md) (même date) : alignement doc, gates Phase A, plan exécuteur Phase B. Révision chiffrée précédente : **2026-04-20** (Phase A PBR). Voir aussi [tickets/](tickets/README.md), [Goals.md](Goals.md).
