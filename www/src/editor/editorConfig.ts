import { EDITOR_MODE_IDS, type EditorModeId, type EditorUiV1 } from './types.js';

const MODE_SET = new Set<string>(EDITOR_MODE_IDS);

/**
 * Valide le JSON `editor-ui.json` (phase-k) et retourne un document typé.
 * Toute ligne ici est couverte par `editorConfig.test.ts`.
 */
export function parseEditorUiV1(raw: unknown): EditorUiV1 {
  if (raw === null || typeof raw !== 'object') {
    throw new Error('editor-ui: document racine attendu (objet)');
  }
  const o = raw as Record<string, unknown>;
  if (o.version !== 1) {
    throw new Error('editor-ui: version 1 requise');
  }
  const shell = o.shell;
  if (shell === null || typeof shell !== 'object') {
    throw new Error('editor-ui: shell manquant');
  }
  const sh = shell as Record<string, unknown>;
  const appearance = sh.appearance;
  if (appearance !== 'light' && appearance !== 'dark') {
    throw new Error('editor-ui: shell.appearance light|dark');
  }
  const layout = sh.layout;
  if (layout === null || typeof layout !== 'object') {
    throw new Error('editor-ui: shell.layout manquant');
  }
  const lo = layout as Record<string, unknown>;
  const railW = lo.railWidthCssPx;
  if (typeof railW !== 'number' || !Number.isFinite(railW) || railW < 32 || railW > 200) {
    throw new Error('editor-ui: shell.layout.railWidthCssPx (32..200)');
  }
  const stage = o.stage;
  if (stage === null || typeof stage !== 'object') {
    throw new Error('editor-ui: stage manquant');
  }
  const st = stage as Record<string, unknown>;
  if (typeof st.title !== 'string' || st.title.length === 0) {
    throw new Error('editor-ui: stage.title (non vide)');
  }
  if (typeof st.defaultBreadcrumb !== 'string') {
    throw new Error('editor-ui: stage.defaultBreadcrumb (string)');
  }
  const modes = o.modes;
  if (!Array.isArray(modes) || modes.length !== 8) {
    throw new Error('editor-ui: exactement 8 modes');
  }
  const out: { id: EditorModeId; label: string; keyHint: string }[] = [];
  for (const m of modes) {
    if (m === null || typeof m !== 'object') {
      throw new Error('editor-ui: entrée mode invalide');
    }
    const e = m as Record<string, unknown>;
    if (typeof e.id !== 'string' || !MODE_SET.has(e.id)) {
      throw new Error('editor-ui: id de mode inconnu');
    }
    if (typeof e.label !== 'string' || e.label.length === 0) {
      throw new Error('editor-ui: label de mode');
    }
    if (typeof e.keyHint !== 'string') {
      throw new Error('editor-ui: keyHint (string, vide autorisé)');
    }
    out.push({ id: e.id as EditorModeId, label: e.label, keyHint: e.keyHint });
  }
  for (let i = 0; i < EDITOR_MODE_IDS.length; i += 1) {
    if (out[i]!.id !== EDITOR_MODE_IDS[i]) {
      throw new Error('editor-ui: ordre des modes doit suivre la spec (build..ship)');
    }
  }
  return {
    version: 1,
    shell: { appearance, layout: { railWidthCssPx: railW } },
    stage: { title: st.title, defaultBreadcrumb: st.defaultBreadcrumb },
    modes: out,
  };
}

/**
 * Raccourcis clavier (maquette v3) : B P S L A I, Espace → Play, pas de touche pour Ship.
 * Retourne le mode ciblé, ou `null` si l’évènement ne correspond pas.
 */
export function keyEventToModeId(
  e: { code: string; key: string; ctrlKey: boolean; altKey: boolean; metaKey: boolean },
  _current: EditorModeId,
): EditorModeId | null {
  if (e.ctrlKey || e.altKey || e.metaKey) {
    return null;
  }
  if (e.code === 'Space') {
    return 'play';
  }
  const k = e.key.length === 1 ? e.key.toLowerCase() : '';
  if (k === 'b') {
    return 'build';
  }
  if (k === 'p') {
    return 'paint';
  }
  if (k === 's') {
    return 'sculpt';
  }
  if (k === 'l') {
    return 'logic';
  }
  if (k === 'a') {
    return 'animate';
  }
  if (k === 'i') {
    return 'light';
  }
  return null;
}
