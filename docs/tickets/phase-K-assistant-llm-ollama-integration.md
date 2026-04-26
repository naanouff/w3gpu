# Phase K (sous-ticket) — Assistant LLM, compatible Ollama (local, optionnel)

| Champ | Valeur |
|-------|--------|
| **ID** | `PHASE-K-ASSISTANT-LLM` |
| **Objectif** | **Intégrer un LLM local** (hébergé sur le poste, typ. via **Ollama** ou un service **compatible** exposé en HTTP sur `127.0.0.1`) afin d’**alimenter l’assistant IA** de l’éditeur : panneau de chat, réponses en streaming, et (en V2) propositions d’édit — le tout **optionnel** : l’éditeur reste pleinement utilisable **sans** LLM (Noop, `enabled: false`). |
| **Note** | Périmètre **éditeur / outillage** ; l’**inférence** n’est **pas** dans le binaire w3d : **sidecar** (Ollama ou équivalent) — **pas** dans le moteur PBR/ECS. Aligné [architecture — Plugins / éditeur](../architecture.md), [Phase K](phase-K-editeur-workspaces.md), priorité [éditeur natif](../../.cursor/rules/w3drs-native-editor-priority.mdc). |
| **Roadmap** | [ROADMAP — Principes d’architecture](../ROADMAP.md#principes-darchitecture-produit-w3drs) (*data-driven* ; *modularité*) |
| **Statut** | **Baseline livrée** (crates + feature + coquille) — **DOD partielle** ; périmètre *produit fini* ci-dessous **à boucler** |
| **Ticket parent** | [Phase K — Éditeur, workspaces, extensions](phase-K-editeur-workspaces.md) |
| **Tickets liés** | [Phase B — éditeur UI/UX v3 hi-fi](phase-B-editor-ui-ux-implementation.md) (✦, panneau chat, nudges) |

## Intention (mise en œuvre de l’objectif)

Réaliser cette alimentation par une intégration **bas niveau** : client **HTTP** (feature Cargo) vers une API de type **Ollama** (`/api/chat`, streaming), **sans** lier l’ouverture de l’éditeur à un service de modèle. **V1** : conversation texte utile à l’assistant. **V2** : propositions d’édit structurées avec validation **avant** toute écriture workspace — *Accept / Reject* explicite, aligné sûreté [phase-transverses](phase-transverses.md).

## Baseline déjà en dépôt (ne pas recréer en douce)

| Composant | Rôle |
|-----------|------|
| `crates/w3drs-assistant-api` | `AssistantBackend`, `NoopBackend`, `ActiveStream`, config `assistant.json` (parse + validation), modèles **V2** (`EditProposalEnvelopeV2`, `validate`) |
| `crates/w3drs-assistant-http` | `OllamaBackend` : `POST {baseUrl}/api/chat` + NDJSON, annulation |
| `editor` (`w3d-editor`) | Feature **`sidecar-ollama`** ; panneau inspector : prompt, flux, annuler — **aucune** écriture disque auto |
| `fixtures/phases/phase-k/assistant.json` + `assistant-edit-proposal-v2.example.json` | Données témoin |

**Commandes de vérification rapide** (voir DOD) :

- `cargo test -p w3drs-assistant-api -p w3drs-assistant-http -p w3d-editor`
- `cargo run -p w3d-editor` (pas d’HTTP, pas d’`ureq`)
- Avec moteur local Ollama : `cargo run -p w3d-editor --features sidecar-ollama` + `enabled: true` dans l’`assistant.json` ad hoc

## Écart architecture (existant → cible)

| Sujet | Existant (baseline) | Cible (ce ticket) |
|-------|---------------------|-------------------|
| **Config** | Fichier à côté de `editor-ui.json` (chemin lancement) | Même idée portée sur **`editor/assistant.json` du workspace** (voir [Goals.md](../Goals.md)) dès que l’ouverture projet = racine réelle — pas de divergence de sémantique `baseUrl` / `model` |
| **Data-driven** | UI assistant **codée** dans l’egui (placeholder) | **Panneau / raccourcis** décrits dans le **layout** (JSON/RON) comme le reste du shell — voir DOD |
| **V2** | Types + validation + **exemple JSON** | **Parse** d’un bloc proposé par l’utilisateur (collage) ou par un futur *tool* ; **diff** sur fichier ; **Apply** = commande unique, traçable, jamais silencieuse |
| **Extension** | Feature Cargo + binaire lié | Option **plugin éditeur** (dylib) si l’[architecture plugins](../architecture.md#architecture-plugins-modulaire) l’impose avant ouverture à des tiers — à trancher dans une PR |
| **Sécurité** | Avertissements conceptuels | **Opt-in** explicite pour l’inclusion de chemins de workspace / contenu de fichiers dans le contexte LLM (case, prévision d’embarquement) + doc menace modèle de menace dans ticket ou `architecture.md` |
| **CI / E2E** | Pas de test réseau Ollama en CI par défaut | Test d’**intégration** **hors** CI (variable d’environ, `#[ignore]`, doc *README* fixture) ou bouchon local — **DOD** |

## Périmètre à compléter (DOD cible intégration produit)

1. **Raccord workspace** : résolution fiable de `…/<workspace>/editor/assistant.json` (et fallback cohérent avec la baseline « même dossier que editor-ui» pour les fixtures).
2. **Layout / thème** : retrait des chaînes « magiques » du seul `app.rs` pour l’assistant vers données versionnables (même logique que shell Phase B / Phase K).
3. **V2 côté UI** : zone *proposition* (JSON) → `validate` → aperçu (résumé) + bouton **Proposer l’edit** (pas d’écriture) puis **Appliquer** (commande w3d unique sur workspace).
4. **Documentation** : paragraphe dans `architecture.md` (éditeur) ou lien depuis Phase K : *assistant = optionnel* ; *Ollama* = *compat API* recommandé ; in-process = hors périmètre actuel.
5. **Intégration / réseau (optionnel CI)** : un test manuel documenté (script ou section fixture) *ou* un test `#[ignore]` + doc pour valider contre un Ollama local.

## DOR (*Definition of Ready*)

- Décision **binaire** : le panneau assistant reste un **module du binaire w3d-editor** jusqu’à ce qu’un *plugin* soit choisi, **ou** critères d’extraction (ABI, signature) listés.
- **Schéma** `assistant.json` (version) figé et référencé par un test de parse (déjà partiellement le cas) + changelog si champs ajoutés (p.ex. *max context*, *system prompt* fichier).
- Maquette / tokens : **alignement** [Phase B UI](phase-B-editor-ui-ux-implementation.md) pour le **FAB** ✦ et le panneau chat (placeholders de la baseline à remplacer, pas l’inverse).

## DOD (*Definition of Done*)

- Tous les critères **Périmètre à compléter** ci-dessus sont remplis **ou** déplacés explicitement en sous-issues liées, avec *Statut* mis à jour dans ce fichier.
- `cargo test` sur `w3drs-assistant-api`, `w3drs-assistant-http`, `w3d-editor` **verts** sur la cible.
- Aucun chemin d’ouverture éditeur **dépendant** d’Ollama (greffer un test d’*smoke* `w3d-editor` sans `sidecar-ollama` en CI si pas déjà fait).
- Mise à jour de **`docs/journal.md`** à la clôture (règle tickets [README](README.md)).

## Outils de validation (DOD)

| Outil | Rôle |
|-------|------|
| `cargo test -p w3drs-assistant-api -p w3drs-assistant-http -p w3d-editor` | Non-régression logique + UI compile |
| `cargo run -p w3d-editor` | Shell sans feature LLM (Noop) |
| `cargo run -p w3d-editor --features sidecar-ollama` + Ollama local | Vérif manuelle ou scénario doc |
| (Option) test `#[ignore] ` ou script curl | Garantit la trace de validation réseau sans enfermer la CI |

## Scène / fixture de test

- **Immédiat** : [`assistant.json`](../../fixtures/phases/phase-k/assistant.json) et [`assistant-edit-proposal-v2.example.json`](../../fixtures/phases/phase-k/assistant-edit-proposal-v2.example.json).
- **Cible** : le même contenu, référencé depuis un **workspace** témoin `fixtures/phases/phase-k/workspace/` quand l’ouverture réelle du workspace existera (alignement [Phase K — scène de test](phase-K-editeur-workspaces.md)).

## Références externes (comportement attendu côté poste)

- **Ollama** : service local typique `http://127.0.0.1:11434` — **ne pas** exposer par défaut sur l’Internet ; pare-feu poste.
- Toute autre API **Ollama-compatible** sur le même style `POST /api/chat` + streaming peut remplir `baseUrl` dans la config, sous réserve de parité (documenter les écarts s’il y a lieu).

---

*Création : 2026-04. Baseline code : w3drs-assistant-* + `editor` avec feature `sidecar-ollama`.*
