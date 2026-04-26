import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { PHASE_A_GATE_BASENAMES } from '../../vite-plugin-phase-a-gate';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

/**
 * Rappel : noms = racines attendues sous /phase-a-gate/ (fixtures phase-a) ;
 * id / ordre alignés `examples/pbr-viewer` et `pbr_state::KHRONOS_GLBS`.
 */
describe('viewer-manifest.json', () => {
  it('7 modèles, casque + six chemins /phase-a-gate/* cohérents', () => {
    const manifestPath = path.resolve(__dirname, '../../public/phase-a/viewer-manifest.json');
    const j = JSON.parse(fs.readFileSync(manifestPath, 'utf8')) as {
      models: { id: string; url: string }[];
    };
    expect(j.models).toHaveLength(7);
    const urls = j.models.map((m) => m.url);
    expect(urls[0]).toBe('/damaged_helmet_source_glb.glb');
    for (let i = 0; i < PHASE_A_GATE_BASENAMES.length; i += 1) {
      const base = PHASE_A_GATE_BASENAMES[i]!;
      expect(urls[i + 1]!).toBe(`/phase-a-gate/${base}`);
    }
  });
});
