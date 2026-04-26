# Extension « hello » (stub Phase K)

Place-holder pour l’**extension tierce** décrite en [Phase K — ticket](../../../../../docs/tickets/phase-K-editeur-workspaces.md) : chargement dynamique (DLL / wasm) **hors** du binaire cœur — l’**ABI** et l’`entry` relèvent d’une future PR (voir [architecture — Plugins](../../../../../docs/architecture.md)).

- **Manifeste** : [`plugin.json`](plugin.json) (schéma provisoire aligné sur la section *Extensions éditeur* d’`architecture.md` : `id`, `name`, `version`).

Quand l’hôte d’extension existe : ce dossier servira de **première** cible d’essai (hook documenté, pas de logique lourde).
