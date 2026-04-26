export const EDITOR_MODE_IDS = [
  'build',
  'paint',
  'sculpt',
  'logic',
  'animate',
  'light',
  'play',
  'ship',
] as const;

export type EditorModeId = (typeof EDITOR_MODE_IDS)[number];

export type EditorUiV1 = {
  version: 1;
  shell: {
    appearance: 'light' | 'dark';
    layout: { railWidthCssPx: number };
  };
  stage: { title: string; defaultBreadcrumb: string };
  modes: { id: EditorModeId; label: string; keyHint: string }[];
};
