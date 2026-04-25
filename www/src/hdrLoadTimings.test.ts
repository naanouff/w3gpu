import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  loadHdrWithTimings,
  roundMs,
  type HdrLoadEngineLike,
} from './hdrLoadTimings.js';

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe('roundMs', () => {
  it('arrondit à 2 décimales', () => {
    expect(roundMs(1.234)).toBe(1.23);
    expect(roundMs(1.235)).toBe(1.24);
    expect(roundMs(0)).toBe(0);
  });
});

describe('loadHdrWithTimings (fonctionnel, mocks fetch + moteur)', () => {
  it('renvoie ok avec octets, temps client et détail wasm', async () => {
    const payload = new Uint8Array([0, 1, 2, 3]);
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(
        new Response(payload.buffer, { status: 200, statusText: 'OK' }),
      ),
    );
    const times = [1000, 1000.4, 1000.4, 1005.0];
    let i = 0;
    const perf = { now: () => times[i++]! };

    const engine: HdrLoadEngineLike = {
      load_hdr: (bytes: Uint8Array) => {
        expect(bytes.length).toBe(4);
        return {
          parse_ms: () => 2,
          ibl_ms: () => 10,
          env_bind_ms: () => 0.5,
          total_ms: () => 12.5,
          free: () => undefined,
        };
      },
    };

    const r = await loadHdrWithTimings(engine, '/fake.hdr', perf);
    expect(r.ok).toBe(true);
    if (r.ok) {
      expect(r.clientBytes).toBe(4);
      expect(r.sourceBytes.length).toBe(4);
      expect(r.clientFetchAndBufferMs).toBe(0.4);
      expect(r.clientWasmCallWallMs).toBe(4.6);
      expect(r.wasm).toEqual({
        parseMs: 2,
        iblMs: 10,
        envBindMs: 0.5,
        totalMs: 12.5,
      });
    }
  });

  it('renvoie fetch_not_ok si le serveur ne renvoie pas 2xx', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(new Response(null, { status: 404 })),
    );
    const engine: HdrLoadEngineLike = {
      load_hdr: () => {
        throw new Error('ne doit pas être appelé');
      },
    };
    const r = await loadHdrWithTimings(engine, '/missing.hdr');
    expect(r.ok).toBe(false);
    if (!r.ok) {
      expect(r.reason).toBe('fetch_not_ok');
      expect(r.message).toContain('404');
    }
  });

  it('renvoie load_hdr si le moteur lève', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(
        new Response(new Uint8Array([1]).buffer, { status: 200 }),
      ),
    );
    const engine: HdrLoadEngineLike = {
      load_hdr: () => {
        throw new Error('hdr invalide');
      },
    };
    const r = await loadHdrWithTimings(engine, '/x.hdr');
    expect(r.ok).toBe(false);
    if (!r.ok) {
      expect(r.reason).toBe('load_hdr');
      expect(r.message).toBe('hdr invalide');
    }
  });

  it('appelle free() sur les stats wasm', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(
        new Response(new Uint8Array(1).buffer, { status: 200 }),
      ),
    );
    const free = vi.fn();
    const engine: HdrLoadEngineLike = {
      load_hdr: () => ({
        parse_ms: () => 0,
        ibl_ms: () => 0,
        env_bind_ms: () => 0,
        total_ms: () => 0,
        free,
      }),
    };
    const r = await loadHdrWithTimings(engine, '/a.hdr');
    expect(r.ok).toBe(true);
    expect(free).toHaveBeenCalledTimes(1);
  });
});
