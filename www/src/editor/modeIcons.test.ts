import { describe, expect, it } from 'vitest';
import { EDITOR_MODE_IDS } from './types.js';
import { modeIconSvg } from './modeIcons.js';

describe('modeIconSvg', () => {
  it('retourne un SVG par mode (chaque clé de la matrice)', () => {
    for (const id of EDITOR_MODE_IDS) {
      const s = modeIconSvg(id);
      expect(s).toMatch(/<svg/);
    }
  });
});
