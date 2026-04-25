/**
 * Mesures HDR côté client (fetch + buffer) et WASM (`HdrLoadStats`) — partagé entre
 * le viewer et la suite de tests fonctionnels.
 */

export type HdrWasmTimings = {
  parseMs: number;
  iblMs: number;
  envBindMs: number;
  totalMs: number;
};

export type HdrLoadTimingsOk = {
  ok: true;
  clientFetchAndBufferMs: number;
  clientWasmCallWallMs: number;
  clientBytes: number;
  /** Copie du buffer passé à `load_hdr` (re-générer l’IBL si `ibl_tier` change). */
  sourceBytes: Uint8Array;
  wasm: HdrWasmTimings;
};

export type HdrLoadTimingsErr = {
  ok: false;
  reason: 'fetch_not_ok' | 'load_hdr';
  message?: string;
};

export type HdrLoadTimingsResult = HdrLoadTimingsOk | HdrLoadTimingsErr;

/** Sous-ensemble de l’API `W3drsEngine` / `HdrLoadStats` (tests sans import du pkg). */
export interface HdrLoadStatsLike {
  parse_ms(): number;
  ibl_ms(): number;
  env_bind_ms(): number;
  total_ms(): number;
  free(): void;
}

export interface HdrLoadEngineLike {
  load_hdr(bytes: Uint8Array): HdrLoadStatsLike;
}

export function roundMs(x: number): number {
  return Math.round(x * 100) / 100;
}

/**
 * Télécharge un `.hdr`, appelle `engine.load_hdr`, renvoie les durées (client + détail WASM).
 * `load_hdr` peut lever (erreur propagée côté JS depuis Rust).
 */
export async function loadHdrWithTimings(
  engine: HdrLoadEngineLike,
  hdrUrl: string,
  perf: Pick<Performance, 'now'> = globalThis.performance,
): Promise<HdrLoadTimingsResult> {
  const t0 = perf.now();
  const hdrRes = await fetch(hdrUrl);
  if (!hdrRes.ok) {
    return {
      ok: false,
      reason: 'fetch_not_ok',
      message: `HTTP ${String(hdrRes.status)}`,
    };
  }
  const ab = await hdrRes.arrayBuffer();
  const clientFetchAndBufferMs = roundMs(perf.now() - t0);
  const bytes = new Uint8Array(ab);
  const tWasm0 = perf.now();
  let wasmStats: HdrLoadStatsLike;
  try {
    wasmStats = engine.load_hdr(bytes);
  } catch (e) {
    return {
      ok: false,
      reason: 'load_hdr',
      message: e instanceof Error ? e.message : String(e),
    };
  }
  const clientWasmCallWallMs = roundMs(perf.now() - tWasm0);
  const wasm: HdrWasmTimings = {
    parseMs: roundMs(wasmStats.parse_ms()),
    iblMs: roundMs(wasmStats.ibl_ms()),
    envBindMs: roundMs(wasmStats.env_bind_ms()),
    totalMs: roundMs(wasmStats.total_ms()),
  };
  wasmStats.free();
  return {
    ok: true,
    clientFetchAndBufferMs,
    clientWasmCallWallMs,
    clientBytes: bytes.byteLength,
    sourceBytes: bytes,
    wasm,
  };
}
