import type { SceneHandles } from '../viewer/scene.js';

/** Ligne d’outliner (même provenance qu’un futur miroir ECS côté WASM). */
export type OutlinerRow = {
  key: string;
  /** `null` = en-tête de section non sélectionnable. */
  entityId: number | null;
  label: string;
  /** Indentation 0 = racine, 1+ = enfants. */
  depth: number;
};

export function outlinerRowsFromScene(scene: SceneHandles, modelName: string): OutlinerRow[] {
  const rows: OutlinerRow[] = [];
  rows.push({ key: 'hdr-scene', entityId: null, label: 'Scene', depth: 0 });
  rows.push({ key: 'hdr-cam', entityId: scene.cameraEntity, label: 'Active Camera', depth: 1 });
  rows.push({ key: 'hdr-model', entityId: null, label: `Model (${modelName})`, depth: 1 });
  for (let i = 0; i < scene.modelEntities.length; i += 1) {
    const e = scene.modelEntities[i]!;
    rows.push({ key: `m-${i}`, entityId: e, label: `Mesh ${String(i)}`, depth: 2 });
  }
  rows.push({ key: 'hdr-level', entityId: null, label: 'Level', depth: 1 });
  rows.push({ key: 'w-wall', entityId: scene.wallEntity, label: 'Backdrop', depth: 2 });
  rows.push({ key: 'f-floor', entityId: scene.floorEntity, label: 'Ground', depth: 2 });
  return rows;
}

function defaultSelectionForScene(s: SceneHandles): number | null {
  if (s.modelEntities.length > 0) {
    return s.modelEntities[0]!;
  }
  return s.cameraEntity;
}

function findRow(rows: OutlinerRow[], eid: number | null): OutlinerRow | undefined {
  if (eid === null) {
    return undefined;
  }
  return rows.find((r) => r.entityId === eid);
}

/**
 * Construit l’outliner Build, met à jour le hint viewport (sélection sans picking 3D).
 * La surbrillance 3D / *outline* nécessitera une API moteur dédiée.
 */
export function createOutlinerController(
  body: HTMLElement,
  hint: HTMLElement,
  onSelect: (entityId: number | null) => void,
): {
  sync: (scene: SceneHandles, modelName: string) => void;
  getSelectedEntity: () => number | null;
} {
  let rows: OutlinerRow[] = [];
  let selected: number | null = null;

  const setHint = (): void => {
    if (selected === null) {
      hint.textContent = '';
      hint.setAttribute('hidden', '');
      return;
    }
    const row = findRow(rows, selected);
    const label = row != null ? row.label : `entity ${String(selected)}`;
    hint.textContent = `Sélection : ${label} (entité ${String(selected)}) — outline 3D : API moteur à venir`;
    hint.removeAttribute('hidden');
  };

  const render = (): void => {
    body.replaceChildren();
    for (const r of rows) {
      const el = document.createElement('div');
      el.className = 'w3d-outliner__row';
      el.style.paddingLeft = `calc(8px + ${String(r.depth * 10)}px)`;
      el.dataset.key = r.key;
      if (r.entityId !== null) {
        el.dataset.entity = String(r.entityId);
        el.setAttribute('role', 'button');
        el.tabIndex = 0;
        if (r.entityId === selected) {
          el.classList.add('w3d-outliner__row--sel');
        }
        el.addEventListener('click', () => {
          selected = r.entityId;
          onSelect(selected);
          render();
          setHint();
        });
        el.addEventListener('keydown', (ev) => {
          if (ev.key === 'Enter' || ev.key === ' ') {
            ev.preventDefault();
            selected = r.entityId;
            onSelect(selected);
            render();
            setHint();
          }
        });
      } else {
        el.classList.add('w3d-outliner__row--hdr');
        el.setAttribute('aria-hidden', 'true');
      }
      const span = document.createElement('span');
      span.textContent = r.label;
      el.appendChild(span);
      body.appendChild(el);
    }
    setHint();
  };

  return {
    sync: (scene: SceneHandles, modelName: string): void => {
      rows = outlinerRowsFromScene(scene, modelName);
      selected = defaultSelectionForScene(scene);
      onSelect(selected);
      render();
    },
    getSelectedEntity: (): number | null => selected,
  };
}
