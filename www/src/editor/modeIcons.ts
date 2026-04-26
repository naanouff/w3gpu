import type { EditorModeId } from './types.js';

/** Icônes 24×24 (stroke) alignées sur la maquette v3 — lecture seule, pas d’arbre DOM ici. */
const SVG: Record<EditorModeId, string> = {
  build: '<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="4" y="4" width="16" height="16" rx="2"/></svg>',
  paint:
    '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 20 L14 10 L19 15 L14 20 Z"/><circle cx="17" cy="7" r="2"/></svg>',
  sculpt: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 18 C8 4, 16 22, 20 8"/></svg>',
  logic:
    '<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="6" cy="6" r="2"/><circle cx="18" cy="18" r="2"/><path d="M8 6 L16 18"/></svg>',
  animate:
    '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 18 H20 M6 18 L6 10 L10 13 L14 6 L18 14"/></svg>',
  light:
    '<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="4"/><path d="M12 3 V6 M12 18 V21 M3 12 H6 M18 12 H21"/></svg>',
  play: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 5 L18 12 L8 19 Z"/></svg>',
  ship: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 16 L12 8 L20 16 M12 8 V20"/></svg>',
};

export function modeIconSvg(id: EditorModeId): string {
  return SVG[id];
}
