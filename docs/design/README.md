# Design — maquettes éditeur

## Maquette native « mode-based » (v2)

La première maquette de l’**éditeur natif** Rust / w3gpu repose sur le fichier **`Mode-based v2.html`** (projet **w3gpu editor**).

- **Référence hors dépôt (exemple)** : `c:\Users\utilisateur\Downloads\w3gpu editor\Mode-based v2.html`
- **Dans ce dépôt** : placer une copie nommée `mode-based-v2.html` dans ce dossier (`docs/design/`) pour que les revues de code et la CI puissent s’y référer sans chemin absolu machine.

## Déclinaison web (`www/`)

Le shell **`www/`** implémente une **version allégée** de cette ergonomie (mêmes idées de **modes** et de structure d’UI, surface réduite pour navigateur + WASM). Les écarts par rapport à la maquette HTML doivent être documentés dans les PR qui touchent l’UI web.

## Tests

Les règles de tests (couverture ligne Rust/TS, E2E, **tests fonctionnels client natif** en plus du parcours `www/`) sont dans [CONTRIBUTING.md](../../CONTRIBUTING.md#testing-policy).
