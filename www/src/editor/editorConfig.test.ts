import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { keyEventToModeId, parseEditorUiV1 } from './editorConfig.js';
import type { EditorModeId } from './types.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const pathFixture = path.resolve(__dirname, '../../../fixtures/phases/phase-k/editor-ui.json');

const ev = (code: string, key: string) => ({ code, key, ctrlKey: false, altKey: false, metaKey: false });

describe('parseEditorUiV1 (phase-k editor-ui.json)', () => {
  it('parse le JSON fixture (8 modes, dark, rail 48 — maquette v3 hi-fi)', () => {
    const raw: unknown = JSON.parse(fs.readFileSync(pathFixture, 'utf8'));
    const d = parseEditorUiV1(raw);
    expect(d.modes).toHaveLength(8);
    expect(d.modes[0]!.id).toBe('build');
    expect(d.modes[7]!.id).toBe('ship');
    expect(d.shell.appearance).toBe('dark');
    expect(d.shell.layout.railWidthCssPx).toBe(48);
  });

  it('rejette document invalide (branches d’erreur)', () => {
    const ok: unknown = JSON.parse(fs.readFileSync(pathFixture, 'utf8'));
    expect(() => parseEditorUiV1(null as unknown)).toThrow();
    expect(() => parseEditorUiV1('x')).toThrow();
    expect(() => parseEditorUiV1(1)).toThrow();
    expect(() => parseEditorUiV1({ ...(ok as object), version: 2 })).toThrow();
    expect(() => parseEditorUiV1({ ...(ok as object), shell: 1 })).toThrow();
    const o1 = { ...(ok as { shell: object }) };
    o1.shell = { appearance: 'dark' } as (typeof o1)['shell'];
    expect(() => parseEditorUiV1(o1)).toThrow();
    const o2 = {
      ...(ok as object),
      shell: { appearance: 'light', layout: { railWidthCssPx: 10 } },
    };
    expect(() => parseEditorUiV1(o2)).toThrow();
    expect(() => parseEditorUiV1({ ...(ok as object), stage: 1 })).toThrow();
    const o3 = {
      ...(ok as { stage: { title: string; defaultBreadcrumb: string } }),
      stage: { title: '', defaultBreadcrumb: 'x' },
    };
    expect(() => parseEditorUiV1(o3)).toThrow();
    const o4 = { ...(ok as object), modes: [] };
    expect(() => parseEditorUiV1(o4)).toThrow();
    const o4b = { ...(ok as object), modes: [null] };
    expect(() => parseEditorUiV1(o4b as unknown as Record<string, unknown>)).toThrow();
    const o4c: unknown = (() => {
      const a = (ok as { modes: { id: string; label: string; keyHint: string }[] }).modes;
      return { ...(ok as object), modes: a.map((m) => (m.id === 'build' ? { ...m, keyHint: 1 } : m)) };
    })();
    expect(() => parseEditorUiV1(o4c as Record<string, unknown>)).toThrow();
    const o4d: unknown = (() => {
      const a = (ok as { modes: { id: string; label: string; keyHint: string }[] }).modes;
      return { ...(ok as object), modes: a.map((m) => (m.id === 'build' ? { ...m, label: '' } : m)) };
    })();
    expect(() => parseEditorUiV1(o4d as Record<string, unknown>)).toThrow();
    const o6 = { ...(ok as { stage: { title: string; defaultBreadcrumb: string } }) };
    o6.stage = { title: 't', defaultBreadcrumb: 1 } as unknown as (typeof o6)['stage'];
    expect(() => parseEditorUiV1(o6)).toThrow();
    const o5 = {
      ...(ok as { modes: { id: string; label: string; keyHint: string }[] }),
      modes: (ok as { modes: { id: string; label: string; keyHint: string }[] }).modes.map(
        (m) => (m.id === 'build' ? { ...m, id: 'xy' } : m),
      ),
    };
    expect(() => parseEditorUiV1(o5)).toThrow();
    const wrongOrder: unknown = (() => {
      const a = (ok as { modes: { id: string; label: string; keyHint: string }[] }).modes;
      return { ...(ok as object), modes: [a[1]!, a[0]!, ...a.slice(2)] };
    })();
    expect(() => parseEditorUiV1(wrongOrder)).toThrow();
    const badRail: unknown = {
      ...(ok as object),
      shell: { appearance: 'light', layout: { railWidthCssPx: Number.NaN } },
    };
    expect(() => parseEditorUiV1(badRail)).toThrow();
    const badAp: unknown = { ...(ok as object), shell: { appearance: 'grey', layout: (ok as { shell: { layout: object } }).shell.layout } };
    expect(() => parseEditorUiV1(badAp)).toThrow();
    const badVer: unknown = { ...(ok as object), version: 0.5 };
    expect(() => parseEditorUiV1(badVer)).toThrow();
  });

  it('valide l’apparence light (branche appearance)', () => {
    const raw: unknown = JSON.parse(fs.readFileSync(pathFixture, 'utf8'));
    const p = (raw as { shell: { appearance: string } & object }).shell;
    p.appearance = 'light';
    const d = parseEditorUiV1(raw);
    expect(d.shell.appearance).toBe('light');
  });
});

describe('keyEventToModeId', () => {
  it('B P S L A I, Espace → play; modifieurs et touche inconnue → null', () => {
    const cur: EditorModeId = 'build';
    expect(keyEventToModeId(ev('KeyB', 'b'), cur)).toBe('build');
    expect(keyEventToModeId(ev('KeyP', 'p'), cur)).toBe('paint');
    expect(keyEventToModeId(ev('KeyS', 's'), cur)).toBe('sculpt');
    expect(keyEventToModeId(ev('KeyL', 'l'), cur)).toBe('logic');
    expect(keyEventToModeId(ev('KeyA', 'a'), cur)).toBe('animate');
    expect(keyEventToModeId(ev('KeyI', 'i'), cur)).toBe('light');
    expect(keyEventToModeId(ev('Space', ' '), cur)).toBe('play');
    expect(
      keyEventToModeId({ code: 'KeyA', key: 'a', ctrlKey: true, altKey: false, metaKey: false }, cur),
    ).toBeNull();
    expect(
      keyEventToModeId({ code: 'KeyA', key: 'a', ctrlKey: false, altKey: true, metaKey: false }, cur),
    ).toBeNull();
    expect(
      keyEventToModeId({ code: 'KeyA', key: 'a', ctrlKey: false, altKey: false, metaKey: true }, cur),
    ).toBeNull();
    expect(keyEventToModeId(ev('KeyJ', 'j'), cur)).toBeNull();
  });
});
