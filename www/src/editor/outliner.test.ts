// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { createOutlinerController, outlinerRowsFromScene } from './outliner.js';
import type { SceneHandles } from '../viewer/scene.js';

const fake: SceneHandles = {
  cameraEntity: 1,
  modelEntities: [10, 20],
  wallEntity: 30,
  floorEntity: 40,
};

describe('outlinerRowsFromScene', () => {
  it('retourne Scene, caméra, meshes numérotés, Level, backdrop, sol', () => {
    const rows = outlinerRowsFromScene(fake, 't');
    const labels = rows.map((r) => r.label);
    expect(labels[0]).toBe('Scene');
    expect(labels).toContain('Active Camera');
    expect(labels).toContain('Model (t)');
    expect(labels).toContain('Mesh 0');
    expect(labels).toContain('Mesh 1');
    expect(labels).toContain('Level');
    expect(labels).toContain('Backdrop');
    expect(labels).toContain('Ground');
    const mesh0 = rows.find((r) => r.key === 'm-0');
    expect(mesh0?.entityId).toBe(10);
    const hdr = rows.filter((r) => r.entityId === null).length;
    expect(hdr).toBeGreaterThan(0);
  });
});

describe('createOutlinerController', () => {
  it('sync: lignes cliquables avec data-entity, hint actif', () => {
    const body = document.createElement('div');
    const hint = document.createElement('div');
    const c = createOutlinerController(body, hint, () => {});
    c.sync(fake, 'demo');
    expect(body.querySelectorAll('[data-entity]').length).toBeGreaterThan(0);
    expect(c.getSelectedEntity()).not.toBeNull();
    expect((hint.textContent ?? '').length).toBeGreaterThan(0);
    expect(hint.hasAttribute('hidden')).toBe(false);
  });
});
