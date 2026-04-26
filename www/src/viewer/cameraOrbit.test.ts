import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  applyOrbitAccumToEngine,
  createOrbitInputAccumulator,
} from './cameraOrbit.js';

afterEach(() => {
  vi.restoreAllMocks();
});

describe('cameraOrbit', () => {
  it('applyOrbitAccumToEngine transmet les accumulations puis remet à zéro', () => {
    const acc = createOrbitInputAccumulator();
    acc.pX = 2;
    acc.pY = -1;
    acc.sX = 0;
    acc.sY = 0;
    acc.mX = 0;
    acc.mY = 0;
    acc.wheel = 0.5;

    const apply = vi.fn();
    const engine = { applyOrbitInput: apply } as import('../../pkg/w3drs_wasm.js').W3drsEngine;

    applyOrbitAccumToEngine(engine, acc);
    expect(apply).toHaveBeenCalledWith(2, -1, 0, 0, 0, 0, 0.5);
    expect(acc.pX).toBe(0);
    expect(acc.pY).toBe(0);
    expect(acc.wheel).toBe(0);
  });
});
