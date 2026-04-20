# Phase J — Réseau & gameplay

| Champ | Valeur |
|-------|--------|
| **ID** | `PHASE-J` |
| **Roadmap** | [ROADMAP § Phase J](../ROADMAP.md) |
| **Statut** | À faire |

## Axes prioritaires

- **Data-driven** : protocole messages, prefabs réseau, scénarios démo en **fichiers** ; pas de coordonnées spawn uniquement en dur.
- **Multithreading** : I/O réseau **hors** thread render ; file messages bornée testée.
- **Modularité** : crate `w3drs-net` (indicatif) sans dépendre du renderer ; gameplay « thin » en exemples.

## Écart architecture (existant → cible)

- **Existant** : **aucun** protocole réseau ni démo P2P dans w3drs.
- **Cible** : données réseau + scène **2 clients** reproductible ; transport hors thread render.
- **Ajustement** : encart *Plugins / services* ou section dédiée dans `architecture.md` quand le modèle est choisi.

## Périmètre

P2P + relay, ECS gameplay, démo 2 clients.


## Scène ou projet de test (validation fonctionnelle)

Chaque livraison doit inclure ou étendre **un projet de test versionné** permettant la **validation fonctionnelle** des fonctionnalités de cette phase (répétable, même sur CI dès que l’infra le permet).

| Champ | Valeur |
|-------|--------|
| **ID fixture** | phase-j |
| **Chemin cible** | fixtures/phases/phase-j/ (racine dépôt **w3drs/** ; créer au premier jalon — voir [convention](../../fixtures/phases/README.md)) |
| **Rôle** | Données protocole + scène 2 joueurs ; valide sync spawn/déplacement reproductible. |

### Évolution (ROADMAP)

| Moment | Attendu |
|--------|---------|
| **Aujourd’hui** | Dossier **scène / projet** + README.md : prérequis, commandes (cargo xtask client, www), point d’entrée (argument CLI, env, ou config). |
| **À terme** | **Workspace éditeur** + **.w3db** : chargement **natif** et **web** du **même** paquet pour QA sans divergence de données. |

### Description prescrite de la scène v0 (rédigée dès maintenant)

Spec de `fixtures/phases/phase-j/` : **2 clients** reproductibles.

| Élément | Contenu attendu |
|---------|-----------------|
| `README.md` | Lancer relay mock + deux instances ; timeouts numériques. |
| `network_demo.json` | Spawn positions ; message types ; ordre des ticks de test. |
| `expected.md` | État monde identique sur les deux peers après séquence fixe ; profondeur max file messages. |

### Critères de scène (DOR / DOD)

- **DOR** : [ ] fixtures/phases/phase-j/ décrit dans le README (ou PR) avec reproduction **documentée** ; hachages / LFS pour gros binaires.
- **DOD** : [ ] au moins un **test** (cargo test / E2E) **référence** ce chemin ; toute validation manuelle = **checklist** copiable dans la PR ; natif et web utilisent les **mêmes** assets lorsque les deux cibles sont dans le périmètre.


---

## Definition of Ready (DOR)

- [ ] Spéc message v0 + machine d’état documentée.
- [ ] Script ou `docker compose` pour **relay de test** reproductible (port fixe, graine).
- [ ] Base CI verte.

---

## Definition of Done (DOD)

- [ ] Test d’intégration **2 clients in-process** ou **2 process** : spawn + déplacement ; **états finaux identiques** sur les deux après séquence fixe (timeouts numériques).
- [ ] Charge : file messages ne dépasse pas **profondeur max** (assert) sous rafale documentée.
- [ ] Pas de blocage render : métrique thread render (échantillons) < seuil pendant sync réseau simulée.

### Outils de validation

| Outil | Rôle | Seuil |
|-------|------|--------|
| `cargo test` | Scénario 2 peers / mock transport | 0 échec, déterministe. |
| `tokio` / `std` test time | Timeouts | Valeurs fixes dans test. |
| `cargo xtask check` | wasm + native selon scope | Vert. |
| Couverture | Sérialisation + handlers | Seuil diff. |

---

## Journal

- [ ] [`../journal.md`](../journal.md) : protocole version, résultats 2-clients, limites connues.
