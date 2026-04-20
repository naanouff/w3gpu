# Phase I — Rendu hybride (raster / path)

| Champ | Valeur |
|-------|--------|
| **ID** | `PHASE-I` |
| **Roadmap** | [ROADMAP § Phase I](../ROADMAP.md) |
| **Statut** | À faire (optionnelle) |

## Axes prioritaires

- **Data-driven** : graphe ou preset de rendu **hybride** en data ; commutation raster/path sans recompile.
- **Multithreading** : dénoise / tile path sur worker pools ; ne pas bloquer UI thread natif au-delà du seuil documenté.
- **Modularité** : chemin path trace derrière **feature** `path-tracer` (exemple) ; OIDN ou autre derrière trait.

## Écart architecture (existant → cible)

- **Existant** : pipeline **raster** uniquement ; pas de chemin path / denoise.
- **Cible** : preset hybride **data** + métriques SSIM / ref ; natif vs WASM documenté.
- **Ajustement** : section *Diagrammes* ou *Renderer* dans `architecture.md` pour le flux hybride.

## Périmètre

Path trace / denoise, commutation pour HQ, alignement w3dts `HYBRID_RASTER_PATHTRACE_PLAN`.


## Scène ou projet de test (validation fonctionnelle)

Chaque livraison doit inclure ou étendre **un projet de test versionné** permettant la **validation fonctionnelle** des fonctionnalités de cette phase (répétable, même sur CI dès que l’infra le permet).

| Champ | Valeur |
|-------|--------|
| **ID fixture** | phase-i |
| **Chemin cible** | fixtures/phases/phase-i/ (racine dépôt **w3drs/** ; créer au premier jalon — voir [convention](../../fixtures/phases/README.md)) |
| **Rôle** | Scène simplifiée + preset hybride ; valide SSIM / image de référence raster vs path. |

### Évolution (ROADMAP)

| Moment | Attendu |
|--------|---------|
| **Aujourd’hui** | Dossier **scène / projet** + README.md : prérequis, commandes (cargo xtask client, www), point d’entrée (argument CLI, env, ou config). |
| **À terme** | **Workspace éditeur** + **.w3db** : chargement **natif** et **web** du **même** paquet pour QA sans divergence de données. |

### Description prescrite de la scène v0 (rédigée dès maintenant)

Spec de `fixtures/phases/phase-i/` : scène **simple** + preset hybride.

| Élément | Contenu attendu |
|---------|-----------------|
| `README.md` | Natif vs WASM (skip documenté) ; temps max par frame path. |
| `scene_minimal.gltf` | Une géométrie + 1 matériau ; lumières figées. |
| `hybrid_preset.json` | Mode raster / path / résolution ; nombre de samples ; graine. |
| `ref/` | Image de référence **lossless** + **SSIM** ou diff max autorisée dans `expected.md`. |

### Critères de scène (DOR / DOD)

- **DOR** : [ ] fixtures/phases/phase-i/ décrit dans le README (ou PR) avec reproduction **documentée** ; hachages / LFS pour gros binaires.
- **DOD** : [ ] au moins un **test** (cargo test / E2E) **référence** ce chemin ; toute validation manuelle = **checklist** copiable dans la PR ; natif et web utilisent les **mêmes** assets lorsque les deux cibles sont dans le périmètre.


---

## Definition of Ready (DOR)

- [ ] Scène de référence **simplifiée** partagée avec w3dts (ou dérivée) + **image gold** ou métrique SSIM cible.
- [ ] Décision légale / runtime pour **OIDN** (natif) documentée.
- [ ] Base CI verte.

---

## Definition of Done (DOD)

- [ ] Image générée : **SSIM ≥ seuil** ou **diff pixel** ≤ seuil vs référence (nombres dans PR).
- [ ] Test reproductible : même scène data + graine → même image hash (format lossless).
- [ ] Skip WASM documenté si non applicable, avec tests natifs obligatoires.

### Outils de validation

| Outil | Rôle | Seuil |
|-------|------|--------|
| `cargo test` | Comparateur image / hash | SSIM ou diff max. |
| Binaire bench / capture | `native-triangle` étendu ou démo dédiée | Artefact CI uploadé (optionnel). |
| `xtask check` | Native | Vert ; WASM selon scope. |

---

## Journal

- [ ] [`../journal.md`](../journal.md) : SSIM / métriques, captures, limitations WASM vs natif.
