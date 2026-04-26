import { keyEventToModeId } from './editorConfig.js';
import { modeIconSvg } from './modeIcons.js';
import type { EditorModeId, EditorUiV1 } from './types.js';
import './shell.css';

function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  cls: string,
  opt?: { html?: string; id?: string; ariaLabel?: string },
): HTMLElementTagNameMap[K] {
  const n = document.createElement(tag);
  n.className = cls;
  if (opt?.html !== undefined) {
    n.innerHTML = opt.html;
  }
  if (opt?.id) {
    n.setAttribute('id', opt.id);
  }
  if (opt?.ariaLabel) {
    n.setAttribute('aria-label', opt.ariaLabel);
  }
  return n;
}

export type MountedEditorShell = {
  getMode: () => EditorModeId;
  setMode: (m: EditorModeId) => void;
  onKeyNavigateMode: (e: KeyboardEvent) => void;
  root: HTMLElement;
  sidePanelHost: HTMLElement;
  setStageMeta: (title: string, crumb: string) => void;
  applyConfig: (doc: EditorUiV1) => void;
};

const appearanceToHtmlClass = (a: 'light' | 'dark'): 'w3d-ui-light' | 'w3d-ui-dark' =>
  a === 'light' ? 'w3d-ui-light' : 'w3d-ui-dark';

/**
 * Construit le shell éditeur (rail 8 modes, stage) dans l’hôte, panneau viewer dans
 * `#w3d-side` (même hôte côté inspector qu’en Build).
 */
export function mountEditorShell(
  appHost: HTMLElement,
  config: EditorUiV1,
  initialMode: EditorModeId = 'build',
): MountedEditorShell {
  let current: EditorModeId = initialMode;
  const railButtons = new Map<EditorModeId, HTMLButtonElement>();

  const setRailSelection = (m: EditorModeId): void => {
    for (const [id, btn] of railButtons) {
      btn.setAttribute('aria-pressed', id === m ? 'true' : 'false');
    }
  };

  const setEditorModeLayout = (m: EditorModeId, doc: EditorUiV1): void => {
    editor.classList.remove('w3d-editor--play', 'w3d-editor--build');
    if (m === 'play') {
      editor.classList.add('w3d-editor--play');
    } else if (m === 'build') {
      editor.classList.add('w3d-editor--build');
    }
    const label = doc.modes.find((x) => x.id === m)?.label ?? m;
    if (m === 'build' || m === 'play') {
      overlay.setAttribute('hidden', '');
    } else {
      overlay.textContent = `${String(label)} — jalon maquette (moteur actif)`;
      overlay.removeAttribute('hidden');
    }
    if (m === 'play') {
      hud.removeAttribute('hidden');
    } else {
      hud.setAttribute('hidden', '');
    }
  };

  const titleEl = h('h1', 'w3d-stage__title');
  const crumbEl = h('p', 'w3d-stage__crumb');

  const applyDoc = (doc: EditorUiV1): void => {
    document.documentElement.classList.remove('w3d-ui-light', 'w3d-ui-dark');
    document.documentElement.classList.add(appearanceToHtmlClass(doc.shell.appearance));
    document.documentElement.style.setProperty('--w3d-rail', `${String(doc.shell.layout.railWidthCssPx)}px`);
    titleEl.textContent = doc.stage.title;
    crumbEl.textContent = doc.stage.defaultBreadcrumb;
  };

  const editor = h('div', 'w3d-editor', { id: 'w3d-editor' });
  const nav = h('nav', 'w3d-rail', { ariaLabel: "Modes d'édition" });
  nav.appendChild(h('div', 'w3d-rail__brand', { html: 'w3d' }));

  for (const m of config.modes) {
    const btn = h('button', 'w3d-rail__btn', {
      html: `${modeIconSvg(m.id)}<span class="w3d-rail__name">${m.label}</span>${m.keyHint
        ? `<span class="w3d-rail__key" aria-label="raccourci ${m.keyHint}">${m.keyHint}</span>`
        : ''}`,
    }) as HTMLButtonElement;
    btn.type = 'button';
    btn.setAttribute('aria-pressed', 'false');
    btn.dataset.mode = m.id;
    btn.setAttribute('aria-label', `${m.label} (${m.id})`);
    btn.addEventListener('click', () => {
      setMode(m.id);
    });
    railButtons.set(m.id, btn);
    nav.appendChild(btn);
  }

  const head = h('header', 'w3d-stage__head');
  head.appendChild(titleEl);
  head.appendChild(crumbEl);

  const body = h('div', 'w3d-stage__body') as HTMLElement;

  const surface = h('div', 'w3d-surface') as HTMLElement;
  const outliner = h('aside', 'w3d-outliner', { ariaLabel: "Hiérarchie d'entités (placeholder)" });
  outliner.innerHTML =
    '<h3>Outliner</h3>' +
    '<div class="w3d-outliner__row w3d-outliner__row--sel"><span>Scene</span></div>' +
    '<div class="w3d-outliner__row">Mesh root</div>';

  const view = h('div', 'w3d-viewport', { id: 'w3d-canvas-wrap', ariaLabel: 'Viewport 3D' });
  const canvas = h('canvas', 'canvas', { id: 'w3drs-canvas' }) as HTMLCanvasElement;
  canvas.setAttribute('width', '800');
  canvas.setAttribute('height', '600');
  const overlay = h('div', 'w3d-mode-overlay', { id: 'w3d-mode-overlay' });
  overlay.setAttribute('hidden', '');
  view.appendChild(canvas);
  view.appendChild(overlay);

  const sidePanelHost = h('div', 'w3d-inspector', { id: 'w3d-side' });

  surface.appendChild(outliner);
  surface.appendChild(view);
  surface.appendChild(sidePanelHost);
  body.appendChild(surface);

  const hud = h('div', 'w3d-play__hud', { id: 'w3d-play-hud' }) as HTMLElement;
  hud.setAttribute('role', 'status');
  hud.setAttribute('aria-live', 'polite');
  hud.setAttribute('hidden', '');
  hud.innerHTML = 'Lecture · <kbd>Esc</kbd> revenir en Build';
  body.appendChild(hud);

  const stage = h('div', 'w3d-stage');
  stage.appendChild(head);
  stage.appendChild(body);

  const fab = h('button', 'w3d-ai-fab', {}) as HTMLButtonElement;
  fab.type = 'button';
  fab.setAttribute('aria-label', "Assistant (non câblé)");
  fab.textContent = '✦';
  fab.addEventListener('click', () => {
    // jalon : pas de câblage produit
  });

  editor.appendChild(nav);
  editor.appendChild(stage);
  editor.appendChild(fab);
  appHost.appendChild(editor);

  const refDoc = { current: config } as { current: EditorUiV1 };

  const setMode = (m: EditorModeId): void => {
    const lab = refDoc.current.modes.find((x) => x.id === m)?.label ?? m;
    titleEl.textContent = refDoc.current.stage.title;
    crumbEl.textContent = `w3d · ${String(lab)} · pbr sample`;
    current = m;
    setRailSelection(m);
    setEditorModeLayout(m, refDoc.current);
  };

  const onKeyNavigateMode = (e: KeyboardEvent): void => {
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement) {
      return;
    }
    if (e.ctrlKey || e.altKey || e.metaKey) {
      return;
    }
    const next = keyEventToModeId(
      { code: e.code, key: e.key, ctrlKey: e.ctrlKey, altKey: e.altKey, metaKey: e.metaKey },
      current,
    );
    if (next !== null) {
      e.preventDefault();
      setMode(next);
    }
  };

  const getMode = (): EditorModeId => current;

  const setStageMeta = (t: string, c: string): void => {
    titleEl.textContent = t;
    crumbEl.textContent = c;
  };

  const applyConfig = (doc: EditorUiV1): void => {
    refDoc.current = doc;
    applyDoc(doc);
    setMode(getMode());
  };

  const modeSetterForExport = (m: EditorModeId): void => {
    setMode(m);
  };

  applyDoc(config);
  setRailSelection(current);
  setMode(current);

  return {
    getMode,
    setMode: modeSetterForExport,
    onKeyNavigateMode,
    root: editor,
    sidePanelHost,
    setStageMeta,
    applyConfig,
  };
}
