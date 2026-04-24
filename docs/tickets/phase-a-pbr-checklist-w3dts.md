# Phase A — checklist PBR (alignement concept **w3dts**)

Ce document **satisfait le DOR** « checklist PBR alignée w3dts **copiée** ou référencée par URL commit » : la checklist est **copiée ici** sous forme de critères vérifiables. Elle synthétise les exigences de la [ROADMAP § Phase A](../ROADMAP.md) et du ticket [phase-A-pbr-materiaux-gltf.md](phase-A-pbr-materiaux-gltf.md), qui décrivent la parité visée avec le moteur de référence **w3dts** (monorepo TypeScript / WebGPU — voir ROADMAP).

## Référence w3dts (dépôt Git)

La ROADMAP indique un clone **local** voisin `w3dts/` pour les plans détaillés. **Aucun dépôt Git public** n’a été identifié au moment de la préparation pour figer une URL de commit ; dès qu’une URL stable existe, ajouter ici une ligne **Référence pin** :

- URL dépôt : `…`
- Commit : `<sha complet>`
- Fichier ou dossier de *gates* PBR : `…`

Jusqu’à ce pin, la checklist ci-dessous reste la **référence contractuelle w3drs** pour les revues Phase A.

**Implémentation runtime (2026-04)** : le fragment PBR + bake IBL suivent le dépôt local **`w3dts/`** (`shared/pbr_functions.wgsl`, `graph_templates/pbr_master_node.wgsl`, intégrale `ibl/brdf.frag.wgsl`) — écarts documentés dans [`../journal.md`](../journal.md) (lumière unique ; KHR *transmission* / *volume* = modèle monopasse approximatif ; **AO texture** : non ; **Moyen terme** (opaque derrière, refraction) : [ticket § Moyen terme](phase-A-pbr-materiaux-gltf.md)).

---

## Critères fonctionnels (extensions & pipeline)

- [x] **Lecture** des extensions `KHR_materials_*` retenues : facteurs + textures lus dans `w3drs-assets` / `GltfPrimitive` et poussés vers le GPU (uniforms + slots dédiés) — **interprétation shader** : transmission = approximation IBL « à travers » (pas de refraction screen-space) ; specular = F0 diélectrique teinté ; emissive_strength = facteur sur l’émissif ; volume = épaisseur + Beer sur la part « transmise » (approximation).
- [x] **`KHR_materials_anisotropy`** : facteurs + texture optionnelle lus dans `w3drs-assets` ; passe PBR directe anisotrope dans `pbr.wgsl` (IBL speculaire reste isotrope pour l’instant).
- [x] **`KHR_materials_ior`** : lu via `gltf::Material::ior()` ; F0 diélectrique `((n-1)/(n+1))²` dans `pbr.wgsl` (défaut **1.5** sans extension, aligné Khronos). Fixture **`IORTestGrid.glb`** dans le manifeste Phase A ([shortlist](phase-a-khronos-shortlist.md)).
- [x] **`KHR_materials_clearcoat`** : facteurs + textures (`clearcoatTexture` R, `clearcoatRoughnessTexture` G, `texCoord` 0/1) lus dans `w3drs-assets` ; lobes additifs **direct + IBL** dans `pbr.wgsl` (F0 coat IOR 1.5) ; **pas** de `clearcoatNormalTexture` dans cette itération. Fixtures **`ClearCoatCarPaint.glb`** et **`ClearcoatWicker.glb`** ([shortlist](phase-a-khronos-shortlist.md)).
- [x] **`KHR_texture_transform`** : `offset` / `scale` / `rotation` + `texCoord` par `textureInfo` lus dans `w3drs-assets` ; UV transformées par slot dans `pbr.wgsl` (ordre Khronos : translation × rotation × scale). Régression : tests unitaires chargeur + fixture **`TextureTransformTest.glb`** dans le manifeste Phase A ([shortlist](phase-a-khronos-shortlist.md), section *TextureTransformTest*).
- [x] **`KHR_materials_emissive_strength`** : `emissive_strength` lu (crate `gltf`) — facteur w sur `emissive` dans le fragment.
- [x] **`KHR_materials_specular`** : facteurs + `specularTexture` (A) + `specularColorTexture` (sRGB) ; F0 diélectrique = `min(specularColorFactor × tex_rgb × (specularFactor × tex_a), 1)` quand l’extension est présente.
- [x] **`KHR_materials_transmission`** : `transmissionTexture` (R) + facteur — mélange **approximatif** IBL (direction `-R`) sur la part transmise (non métal).
- [x] **`KHR_materials_volume`** (avec transmission) : `thickness*`, `attenuationColor`, `attenuationDistance` + `thicknessTexture` (G) — atténuation type Beer sur la part transmise.
- [x] **Amorcé** : `fixtures/phases/phase-a/materials/default.json` + `w3drs_assets::phase_a_viewer_config` / `parse_phase_a_viewer_config_str_or_default` (IBL diffuse scale, tonemapping) consommés par **`khronos-pbr-sample`** et par **`w3drs-wasm`** (`applyPhaseAViewerConfigJson`, copie servie `www/public/phase-a/materials/default.json`) — étendre aux variantes pipeline / RON si besoin produit.
- [x] **Stratégie shader (périmètre Phase A)** : **A1** — PBR/IBL en **WGSL** direct (`pbr.wgsl` + `MaterialUniforms`), partagé **natif + WASM** (même bind group, même fragment). **A2** (shader graph / nœuds comme w3dts *viewer-editor*) : **hors** livrable Phase A actuelle — prévu côté ROADMAP / outillage éditeur (Phase B / K) ; ne bloque pas la clôture technique PBR.
- [x] **WASM + natif — matrice** (même pipeline matériau ; divergences **volontaires** = une ligne dans la PR) :

| Aspect | `khronos-pbr-sample` (natif) | `w3drs-wasm` + `www/` (WebGPU) |
|--------|-----------------------------|--------------------------------|
| Source matériau glTF | `load_from_bytes` → `GltfPrimitive` + upload textures | Idem `load_from_bytes` |
| Fragment PBR / IBL | `pbr.wgsl` | Même `pbr.wgsl` via `w3drs-renderer` |
| Culling GPU Hi-Z | Réglette (touches / API) | **Espace** : on/off (voir `main.ts`) |
| Config viewer Phase A | `fixtures/.../materials/default.json` | Même **schéma** : `www/public/phase-a/materials/default.json` + `applyPhaseAViewerConfigJson` |
| **Manifeste modèles (URL web)** | `fixtures/.../manifest.json` (chemins disque) | `www/public/phase-a/viewer-manifest.json` (ids alignés, `?m=`) |
| HDR IBL de studio | Fichier workspace (ex. `www/public/studio_small_03_2k.hdr`) | Même asset servi par Vite sous `/` |
| **Textures IBL générées** (bake `IblContext::from_hdr_with_spec`, [`ibl.rs`](../../crates/w3drs-renderer/src/ibl.rs) + [`ibl_spec.rs`](../../crates/w3drs-renderer/src/ibl_spec.rs)) | Résolutions pilotées par **`ibl_tier`** dans la variante active du JSON (`max` par défaut). Voir tableau des préréglages ci-dessous. | *Identique* (même champ JSON, même code) |
| Divergence connue | N/A côté maths PBR | Frame loop / input différents ; perfs **navigateur** non comparables 1:1 au natif en CI headless optionnelle (bake IBL : natif en **parallèle** `rayon` sur 6 faces préfiltrées, WASM en **séquence**) |

*Si une PR ne touche qu’une cible, indiquer « N/A autre cible » sur la ligne concernée.*

#### Préréglages `ibl_tier` (face irradiance² · mip0 pré-filtré² · LUT BRDF²)

| `ibl_tier` | Irradiance | Pré-filtré (mip0) | BRDF LUT | Remarque |
|------------|------------|-------------------|----------|----------|
| `max` (défaut) | 128 | 512 | 256 | Qualité historique ; mips pré-filtré = `log2(size)+1` (ex. 512 → 10 mips) |
| `high` | 64 | 256 | 256 | |
| `medium` | 32 | 128 | 128 | |
| `low` | 32 | 64 | 64 | |
| `min` / `minimum` | 16 | 32 | 32 | Bake le plus court (mesures WASM / itérations) |

**Bench** : `cargo run -p hdr-ibl-bench --release -- --tier=low` (ou `-t medium` puis chemin `.hdr` optionnel). **Web / natif** : éditer `ibl_tier` dans [`default.json`](../../fixtures/phases/phase-a/materials/default.json) (miroir [`www/public/phase-a/materials/default.json`](../../www/public/phase-a/materials/default.json)) puis relancer ; le bake HDR n’a lieu qu’au `load_hdr` (recharger la page côté www).

### Alignement exemple client (natif) / viewer (WASM)

- [x] **Même cœur** : `khronos-pbr-sample` (desktop) et `w3drs-wasm` + `www/` (navigateur) s’exécutent sur **un seul** fragment/vertex PBR et le **même** chargeur glTF (`GltfPrimitive` → `AssetRegistry::upload_*`). Aucun fork shader « web only ».
- [x] **Même spec viewer** : schéma `materials/*.json` Phase A ([`default.json` fixture](../../fixtures/phases/phase-a/materials/default.json) ↔ [`www/public/phase-a/materials/default.json`](../../www/public/phase-a/materials/default.json)) + `emissive`/`tonemap` / `ibl_diffuse_scale` / **`ibl_tier`** (résolution bake IBL) ; aligner le **comment** au besoin, pas les champs, lors des évolutions.
- [x] **Même IBL d’environnement** (réf. studio) : binaire partagé sous `www/public/` (servi côté web) et référencé côté natif depuis le workspace (voir matrice ci-dessus).
- [x] **PR — barème** : modification de `pbr.wgsl`, `MaterialUniforms`, `render_state` (bind group), `GltfPrimitive` / `Material`, ou d’une entrée de l’`#[wasm_bindgen] impl W3drsEngine` → **aussi** : `cargo check -p w3drs-wasm --target wasm32-unknown-unknown` + si surface JS change : `cd www && npm run build:wasm` et smoke `npm run dev` (ou reporter « à faire côté web » explicite dans la PR).
- [x] **Parité *démo* (manifeste web)** : le **natif** enchaîne [`fixtures/.../manifest.json`](../../fixtures/phases/phase-a/manifest.json) ; le **www** charge [`www/public/phase-a/viewer-manifest.json`](../../www/public/phase-a/viewer-manifest.json) (mêmes **`id`**, URLs = chemins Vite) — le gate **DamagedHelmet** y figure par défaut ; `?m=<index>`, **←/→** si plusieurs entrées (rechargement page). **Étendre** le navigateur = copier des `.glb` sous `www/public/…` + les ajouter à `viewer-manifest.json` (les gros binaires restent sous `fixtures/…/glb/` côté Git / LFS, pas dupliqués côté web tant que non copiés).

## Critères visuels / gates (DamagedHelmet + scène minimale)

### Enregistrement DOD (natif + web)

Les critères visuels ci-dessous sont des **contrôles humains**. Pour la **clôture DOD** (Phase A), l’**équipe** exécute la procédure sur **deux cibles** et **décrit l’emargement** dans [phase-a-gates-record.md](phase-a-gates-record.md) (table + date) ; ce fichier est la preuve d’**exécution** jointe à la revue, en plus des cases cochées ici.

### Procédure de reprise (avant de cocher)

1. **Natif** : à la racine `w3drs/`, `cargo xtask client` (ou `cargo run -p khronos-pbr-sample --release`) ; parcourir le [`manifest.json`](../../fixtures/phases/phase-a/manifest.json) avec **←** / **→** ; s’attarder sur l’entrée **DamagedHelmet** (gate) puis la shortlist (anisotropie, clearcoat, IOR, textures…).
2. **Web** : `cd www && npm run dev` (rebuild WASM si le Rust a changé : `npm run build:wasm`) — vérifier scène 5×5 + HDR + `phase-a/materials/default.json` + `phase-a/viewer-manifest.json` (modèle `id` affiché) ; **Espace** = cull Hi-Z ; **←/→** si plusieurs modèles dans le manifeste web ; pas d’erreur **bloquante** dans la console.

Pour le modèle gate ([shortlist](phase-a-khronos-shortlist.md)), en chargeant la scène de test Phase A :

- [x] Pas de **fallback matériau** évident (ex. **magenta** / rose de secours) sur les primitives visibles du gate.
- [x] **IBL** : reflets et éclairage cohérents (pas de sphère « morte » ou normales manifestement inversées sur tout le mesh).
- [x] **Métal / dielectric** : zones métalliques vs isolantes lisibles (roughness perceptible).
- [x] **Textures** : cartes base color / ORM / normales **chargées** (pas de rendu uniforme plat qui indiquerait une chaîne d’import cassée).
- [x] **Transparence / alpha** : si le modèle expose des pixels alpha, pas de tri manifestement faux sur tout l’asset (tolérance documentée si limitation connue).
- [x] **Stabilité** : aucune **panic** ni erreur de validation GPU **bloquante** sur l’asset gate (niveau documenté : layers de validation wgpu si applicable).

*Emargement : [phase-a-gates-record.md](phase-a-gates-record.md) — campagne 2026-04-20.*

## Outils (rappel DOD — hors DOR mais utile en revue)

Voir la section *Outils de validation* du ticket Phase A (`cargo test`, `cargo xtask check`, `clippy`, etc.).

## Bilan — mesures de charge HDR (reproductible)

Fichier de référence IBL : [`www/public/studio_small_03_2k.hdr`](../../www/public/studio_small_03_2k.hdr).  
Le **bake IBL** (irradiance + pré-filtre + LUT) domine le temps (ordre de **secondes** sur machine de dev) ; exécuté **une fois** au chargement de l’environnement, pas par image.

| Contexte | Commande / visibilité | Exemple (session 2026-04-23, Windows, release) |
|----------|------------------------|-----------------------------------------------|
| Binaire `hdr-ibl-bench` | `cargo run -p hdr-ibl-bench --release -- --tier=max` (voir préréglages ci-dessus) — **stderr** `[hdr-ibl-bench] …` | Ex. `tier=max` : `parse_ms ≈ 16` · `ibl_ms ≈ 9820` · `core_ms ≈ 9835` ; avec `tier=min` le bake IBL chute fortement (mesures comparatives) |
| `khronos-pbr-sample` | Au boot : **stderr** `HDR (natif) parse=… ibl=… env_bind=… total=…` (idem `log` si `RUST_LOG=info`) | `parse ≈ 20` · `ibl ≈ 9280` · `env_bind ≈ 0,1` · `total ≈ 9300` (chaîne complète) |
| `www` + WASM | `HdrLoadStats` + `w3drsHdrLoadTimings` + console | Exemple *contributeur* (même `.hdr` ~6,4 Mo) : `parse` ≈ **40** ms, `ibl` ≈ **120,5** s, `env_bind` ≈ **0** ms, `clientFetch+Buffer` ≈ **40** ms. Le **bake IBL** en **navigateur** peut être d’un facteur **×10–×20** (ou plus) vs natif (seul thread, charge UI, moteur WASM). |

*Côté **natif** seul, l’ordre de grandeur usuel est ~**10 s** de bake IBL sur poste de dev. Côté **WASM**, comptes plutôt en **dizaines de secondes à minutes** pour la même opération, selon machine / onglet. Détail chiffré (natif + WASM) : [`../journal.md`](../journal.md) (section *Mesures charge HDR*).*

---

*Checklist rédigée pour le DOR Phase A — 2026-04. Critères visuels / gates fermés — 2026-04-20 (voir [phase-a-gates-record.md](phase-a-gates-record.md)).*
