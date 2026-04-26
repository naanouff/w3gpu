# Attendu bake / extension (DOD Phase K — cible)

Cible documentée **avant** l’outillage de bake : les fichiers ne sont **pas** tous produits par le dépôt aujourd’hui.

| Produit cible | Emplacement | Preuve (future) |
|---------------|-------------|-----------------|
| **`.w3db`** binaire (runtime) | `workspace/dist/*.w3db` | hachage ou test de charge côté `w3drs-assets` / runtime |
| **Hook** extension *hello* | `extensions/hello_stub/` (voir `plugin.json`) | fichier témoin ou log après `register` / `activate` (à définir) |

Dès le premier jalon d’**implémentation** de bake, mettre à jour ce fichier avec noms de fichiers exacts et un **seul** scénario de test reproductible.
