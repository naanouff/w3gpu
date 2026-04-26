import type { W3drsEngine } from '../../pkg/w3drs_wasm.js';

export type OrbitInputAcc = {
  pX: number;
  pY: number;
  sX: number;
  sY: number;
  mX: number;
  mY: number;
  wheel: number;
};

/**
 * Deltas d’une frame, comme `w3drs_input::InputFrame` côté natif.
 * Priorité : milieu > droit > gauche.
 */
function pushPointerDelta(
  buttons: number,
  dX: number,
  dY: number,
  acc: OrbitInputAcc,
): void {
  if ((buttons & 4) !== 0) {
    acc.mX += dX;
    acc.mY += dY;
  } else if ((buttons & 2) !== 0) {
    acc.sX += dX;
    acc.sY += dY;
  } else if ((buttons & 1) !== 0) {
    acc.pX += dX;
    acc.pY += dY;
  }
}

export function createOrbitInputAccumulator(): OrbitInputAcc {
  return { pX: 0, pY: 0, sX: 0, sY: 0, mX: 0, mY: 0, wheel: 0 };
}

/**
 * Orbite (LMB), pan (RMB / MMB), zoom (molette) — aligné sémantiquement sur le viewer PBR natif.
 */
export function bindOrbitInputToCanvas(
  canvas: HTMLCanvasElement,
  acc: OrbitInputAcc,
): void {
  canvas.addEventListener('contextmenu', (e) => {
    e.preventDefault();
  });

  canvas.addEventListener('pointerdown', (e) => {
    if (e.button === 0 || e.button === 1 || e.button === 2) {
      try {
        canvas.setPointerCapture(e.pointerId);
      } catch {
        // ignore
      }
    }
  });

  canvas.addEventListener('pointerup', (e) => {
    try {
      canvas.releasePointerCapture(e.pointerId);
    } catch {
      // ignore
    }
  });

  canvas.addEventListener('pointermove', (e) => {
    if (e.buttons === 0) {
      return;
    }
    pushPointerDelta(e.buttons, e.movementX, e.movementY, acc);
  });

  window.addEventListener(
    'wheel',
    (e) => {
      if (e.target !== canvas) {
        return;
      }
      e.preventDefault();
      acc.wheel += -e.deltaY * 0.01;
    },
    { passive: false },
  );
}

/**
 * À appeler **une fois** par frame, avant `tick` (même sémantique que `InputFrame` natif : une frame).
 */
export function applyOrbitAccumToEngine(
  engine: W3drsEngine,
  acc: OrbitInputAcc,
): void {
  engine.applyOrbitInput(
    acc.pX,
    acc.pY,
    acc.sX,
    acc.sY,
    acc.mX,
    acc.mY,
    acc.wheel,
  );
  acc.pX = 0;
  acc.pY = 0;
  acc.sX = 0;
  acc.sY = 0;
  acc.mX = 0;
  acc.mY = 0;
  acc.wheel = 0;
}
