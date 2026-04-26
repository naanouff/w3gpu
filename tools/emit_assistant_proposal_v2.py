#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Émet sur stdout un JSON `EditProposalEnvelopeV2` valide (sémantique outil/CI, **hors** LLM).

Ne charge aucune clé : sortie reproductible; `w3d-editor` ou tests peuvent l’ingérer
(`w3drs_assistant_api` parse + validate, `apply_proposal_v2` pour disque).
"""
from __future__ import annotations

import json
import sys
from typing import Any


def main() -> None:
    out: dict[str, Any] = {
        "version": 2,
        "id": "emit-python-ci",
        "summary": "Proposition témoin produite par tools/emit_assistant_proposal_v2.py",
        "ops": [
            {
                "op": "configJsonMergePatch",
                "path": "src/default.scene.json",
                "schemaId": "scene",
                "patch": {"label": "Scene label (emitted, apply separately)"},
            }
        ],
    }
    text = json.dumps(out, ensure_ascii=False, indent=2)
    sys.stdout.write(text)
    if not text.endswith("\n"):
        sys.stdout.write("\n")


if __name__ == "__main__":
    main()
