// @vitest-environment happy-dom
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, describe, expect, it } from 'vitest';
import { parseEditorUiV1 } from './editorConfig.js';
import { mountEditorShell } from './mountEditorShell.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const pathFixture = path.resolve(__dirname, '../../../fixtures/phases/phase-k/editor-ui.json');

function loadDoc() {
  return parseEditorUiV1(JSON.parse(fs.readFileSync(pathFixture, 'utf8')) as unknown);
}

describe('mountEditorShell', () => {
  afterEach(() => {
    document.body.replaceChildren();
    document.documentElement.className = '';
    document.documentElement.removeAttribute('style');
  });

  it('cree le rail, le mode actif et le canvas; change de mode (Play) sans erreur', () => {
    const h = document.createElement('div');
    document.body.append(h);
    const s = mountEditorShell(h, loadDoc(), 'build');
    expect(s.getMode()).toBe('build');
    const img = h.querySelector('.w3d-rail__logo') as HTMLImageElement | null;
    expect(img?.getAttribute('src')).toBe('/w3d_logo.svg');
    expect(h.querySelector('#w3drs-canvas')).toBeTruthy();
    expect(h.querySelectorAll('button[aria-pressed="true"]')).toHaveLength(1);
    s.setMode('play');
    expect(s.getMode()).toBe('play');
    expect((h.querySelector('.w3d-play__hud') as HTMLElement | null)?.hasAttribute('hidden')).toBe(
      false,
    );
    s.setMode('build');
  });

  it('onKeyNavigateMode: b → build, Space → play', () => {
    const h = document.createElement('div');
    document.body.append(h);
    const s = mountEditorShell(h, loadDoc(), 'paint');
    s.setMode('light');
    const b = (code: string, key: string) => new KeyboardEvent('keydown', { key, code, bubbles: true });
    s.setMode('logic');
    s.onKeyNavigateMode(
      b('KeyB', 'b'),
    );
    expect(s.getMode()).toBe('build');
    s.onKeyNavigateMode(
      b('Space', ' '),
    );
    expect(s.getMode()).toBe('play');
  });

  it('setStageMeta et applyConfig (tokens rail)', () => {
    const h = document.createElement('div');
    document.body.append(h);
    const s = mountEditorShell(h, loadDoc());
    s.setStageMeta('T1', 'c1');
    const doc = {
      ...loadDoc(),
      shell: { ...loadDoc().shell, layout: { railWidthCssPx: 88 } },
    };
    s.applyConfig(doc);
    expect(String(document.documentElement.style.getPropertyValue('--w3d-rail'))).toContain('88');
  });
});
