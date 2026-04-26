import { describe, expect, it, vi } from 'vitest';
import { reframeOnModelEntities } from './reframeModel.js';

describe('reframeOnModelEntities', () => {
  it('passe un Uint32Array des entités modèle au moteur', () => {
    const reframe = vi.fn();
    const engine = { reframeCameraAroundModelEntities: reframe } as unknown as import(
      '../../pkg/w3drs_wasm.js'
    ).W3drsEngine;

    reframeOnModelEntities(engine, [10, 20, 30]);

    expect(reframe).toHaveBeenCalledOnce();
    const arg = reframe.mock.calls[0]![0] as Uint32Array;
    expect(arg).toBeInstanceOf(Uint32Array);
    expect([...arg]).toEqual([10, 20, 30]);
  });
});
