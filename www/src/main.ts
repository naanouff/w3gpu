import init, { W3drsEngine, w3drsValidateRenderGraphV0 } from '../pkg/w3drs_wasm.js';
import { loadHdrWithTimings } from './hdrLoadTimings.js';
import { DEFAULT_LIVE, type LivePhaseA } from './viewer/phaseAConfig.js';
import { buildViewerScene } from './viewer/scene.js';
import { mountViewerPanel, type LightParams, type ViewerPanelCallbacks } from './viewer/ui.js';

const status = document.getElementById('status')!;
const side = document.getElementById('w3d-side')!;

type ViewerModel = { id: string; url: string };
type ViewerManifest = { version: number; models: ViewerModel[] };

const FALLBACK_URL = '/damaged_helmet_source_glb.glb';
const FALLBACK_ID  = 'damaged_helmet_gate';

let cullEnabled   = true;
let lastModelHint = '';
let nModels       = 1;
let lastHdrBytes: Uint8Array | null  = null;

function readModelIndexOrZero(n: number): number {
  const p = new URLSearchParams(window.location.search);
  const m = p.get('m');
  if (m === null) {
    return 0;
  }
  const i = parseInt(m, 10);
  if (Number.isNaN(i)) {
    return 0;
  }
  return Math.max(0, Math.min(n - 1, i));
}

function goAdjacentModel(n: number, delta: number): void {
  if (n < 2) {
    return;
  }
  const cur  = readModelIndexOrZero(n);
  const next = (cur + delta + n) % n;
  const u    = new URL(window.location.href);
  u.searchParams.set('m', String(next));
  window.location.assign(u.toString());
}

type PhaseAJson = {
  active_variant: string;
  variants: Record<string, {
    ibl_tier?: string;
    ibl_diffuse_scale?: number;
    tonemap?: { exposure?: number; bloom_strength?: number; fxaa?: boolean };
  }>;
};

function liveFromPhaseADoc(j: PhaseAJson): LivePhaseA {
  const v = j.variants[j.active_variant] ?? j.variants.default ?? j.variants[Object.keys(j.variants)[0]!];
  if (v == null) {
    return { ...DEFAULT_LIVE };
  }
  return {
    ibl_tier: v.ibl_tier ?? DEFAULT_LIVE.ibl_tier,
    ibl_diffuse_scale: v.ibl_diffuse_scale ?? DEFAULT_LIVE.ibl_diffuse_scale,
    tonemap: {
      exposure: v.tonemap?.exposure ?? DEFAULT_LIVE.tonemap.exposure,
      bloom_strength: v.tonemap?.bloom_strength ?? DEFAULT_LIVE.tonemap.bloom_strength,
      fxaa: v.tonemap?.fxaa ?? DEFAULT_LIVE.tonemap.fxaa,
    },
  };
}

function applyLight(engine: W3drsEngine, p: LightParams): void {
  engine.setViewerLight(
    p.dirX,
    p.dirY,
    p.dirZ,
    p.colorR,
    p.colorG,
    p.colorB,
    p.directionalIntensity,
    p.ambient,
    p.shadowBias,
  );
}

function updateStatusLine(): void {
  const c = cullEnabled ? 'ON' : 'OFF';
  const el = document.querySelector<HTMLInputElement>('.w3d-cull');
  if (el) {
    el.checked = cullEnabled;
  }
  status.textContent =
    `w3drs v${W3drsEngine.version()}  ${lastModelHint}` +
    `— Hi-Z: ${c}  [Space]  ←/→ modèles  `;
}

function canvasAspect(): number {
  const c = document.getElementById('w3drs-canvas') as HTMLCanvasElement;
  return c.width / Math.max(1, c.height);
}

async function main(): Promise<void> {
  await init();

  let phaseBJsonText: string | null = null;
  try {
    const rg = await fetch('/phase-b/render_graph.json');
    if (rg.ok) {
      phaseBJsonText = await rg.text();
      w3drsValidateRenderGraphV0(phaseBJsonText, 'hdr_color');
      console.info(
        '[w3drs] Phase B: validate OK (w3drsValidateRenderGraphV0, readback=hdr_color)',
      );
    } else {
      console.warn(`[w3drs] Phase B: /phase-b/render_graph.json HTTP ${String(rg.status)}`);
    }
  } catch (e) {
    console.warn('[w3drs] Phase B: graph validate skipped (optional):', e);
  }

  status.textContent = 'Creating engine...';
  const engine = await W3drsEngine.create('w3drs-canvas');

  if (phaseBJsonText != null) {
    try {
      const doc = JSON.parse(phaseBJsonText) as {
        passes?: { shader?: string; kind: string }[];
      };
      const need = new Set<string>();
      for (const p of doc.passes ?? []) {
        if (typeof p.shader === 'string' && p.shader.length > 0) {
          need.add(p.shader);
        }
      }
      const wgsl: Record<string, string> = {};
      for (const relp of need) {
        const sh = await fetch(`/phase-b/${relp}`);
        if (!sh.ok) {
          throw new Error(`WGSL ${relp} HTTP ${String(sh.status)}`);
        }
        wgsl[relp] = await sh.text();
      }
      const sum = engine.w3drsPhaseBGraphRunChecksum(
        phaseBJsonText,
        JSON.stringify(wgsl),
        'hdr_color',
      ) as string;
      console.info(
        `[w3drs] Phase B.5: GPU graph checksum = ${String(sum)} (w3drsPhaseBGraphRunChecksum)`,
      );
    } catch (e) {
      console.warn('[w3drs] Phase B.5: GPU run skipped (optional):', e);
    }
  }

  let initialLive: LivePhaseA = { ...DEFAULT_LIVE };
  try {
    const cfg = await fetch('/phase-a/materials/default.json');
    if (cfg.ok) {
      const txt  = await cfg.text();
      engine.applyPhaseAViewerConfigJson(txt);
      const doc = JSON.parse(txt) as PhaseAJson;
      initialLive = liveFromPhaseADoc(doc);
    }
  } catch (e) {
    console.warn('Phase A viewer config not applied (optional):', e);
  }

  // HDR
  status.textContent = 'Loading environment...';
  try {
    const hdrResult = await loadHdrWithTimings(engine, '/studio_small_03_2k.hdr');
    if (hdrResult.ok) {
      lastHdrBytes = hdrResult.sourceBytes;
      window.w3drsHdrLoadTimings = hdrResult;
      console.info('[w3drs] HDR timing', {
        clientFetchAndBufferMs: hdrResult.clientFetchAndBufferMs,
        clientWasmCallWallMs: hdrResult.clientWasmCallWallMs,
        clientBytes: hdrResult.clientBytes,
        wasm: hdrResult.wasm,
      });
    } else {
      console.warn('HDR load failed:', hdrResult.reason, hdrResult.message ?? '');
    }
  } catch (e) {
    console.warn('HDR load failed, using default IBL:', e);
  }

  // GLB
  let modelUrl   = FALLBACK_URL;
  let modelId    = FALLBACK_ID;
  nModels        = 1;
  let modelIndex = 0;
  try {
    const mRes = await fetch('/phase-a/viewer-manifest.json');
    if (mRes.ok) {
      const man: ViewerManifest = (await mRes.json()) as ViewerManifest;
      if (Array.isArray(man.models) && man.models.length > 0) {
        nModels    = man.models.length;
        modelIndex = readModelIndexOrZero(nModels);
        const m    = man.models[modelIndex] ?? man.models[0]!;
        modelUrl   = m.url;
        modelId    = m.id;
      }
    }
  } catch (e) {
    console.warn('viewer-manifest.json: fallback to DamagedHelmet', e);
  }

  const reloadHdrFromMemory = (reason: string): void => {
    if (lastHdrBytes == null) {
      console.warn('[w3drs] Pas de buffer HDR mémorisé, impossible de recharger IBL (', reason, ')');
      return;
    }
    const stats = engine.load_hdr(lastHdrBytes);
    console.info(`[w3drs] IBL rechargé (${reason}) : ${String(stats.total_ms().toFixed(1))}ms`);
    stats.free();
  };

  const rebuildFromGltf = async (bytes: Uint8Array, hint: string): Promise<void> => {
    status.textContent = 'Chargement GLB…';
    engine.clearSceneForNewGltf();
    const ids = engine.load_gltf(bytes);
    if (ids.length < 2) {
      throw new Error('No primitives in GLB');
    }
    const canvas = document.getElementById('w3drs-canvas') as HTMLCanvasElement;
    const aspect = canvas.width / Math.max(1, canvas.height);
    buildViewerScene(engine, ids, aspect);
    lastModelHint = hint;
    updateStatusLine();
  };

  status.textContent = 'Loading model...';
  const res = await fetch(modelUrl);
  if (!res.ok) {
    throw new Error(`Model HTTP ${res.status} (${modelId})`);
  }
  await rebuildFromGltf(
    new Uint8Array(await res.arrayBuffer()),
    nModels > 1
      ? ` ${modelId}  [m=${String(modelIndex + 1)}/${String(nModels)}]  ←/→  `
      : ` ${modelId}  `,
  );

  const cbs: ViewerPanelCallbacks = {
    onLivePhaseA: (json) => {
      engine.applyPhaseAViewerConfigJson(json);
    },
    onCull: (v) => {
      cullEnabled = v;
      engine.set_cull_enabled(v);
      updateStatusLine();
    },
    onReframe: () => {
      engine.reframeCamera();
    },
    onLight: (p) => {
      applyLight(engine, p);
    },
    onIblTierChanged: () => {
      reloadHdrFromMemory('changement IBL tier');
    },
    onHdrFile: async (file) => {
      const b = new Uint8Array(await file.arrayBuffer());
      lastHdrBytes = b;
      const t0  = performance.now();
      const stats = engine.load_hdr(b);
      const wms   = performance.now() - t0;
      console.info(
        `HDR manuel: wasm=${String(wms.toFixed(1))}ms (parse/ibl/bind ${String(stats.total_ms().toFixed(1))}ms)`,
      );
      stats.free();
    },
    onGlbFile: async (file) => {
      const b   = new Uint8Array(await file.arrayBuffer());
      const id  = file.name.replace(/\.glb$/i, '');
      await rebuildFromGltf(b, ` ${id}  (fichier)  `);
    },
  };
  mountViewerPanel(side, initialLive, cbs);
  updateStatusLine();

  document.addEventListener('keydown', (e) => {
    if (e.code === 'Space') {
      cullEnabled = !cullEnabled;
      engine.set_cull_enabled(cullEnabled);
      const box = document.querySelector<HTMLInputElement>('.w3d-cull');
      if (box) {
        box.checked = cullEnabled;
      }
      updateStatusLine();
      e.preventDefault();
      return;
    }
    if (e.code === 'ArrowLeft') {
      goAdjacentModel(nModels, -1);
      e.preventDefault();
    }
    if (e.code === 'ArrowRight') {
      goAdjacentModel(nModels, 1);
      e.preventDefault();
    }
  });

  const onResize = (): void => {
    const canvas    = document.getElementById('w3drs-canvas') as HTMLCanvasElement;
    const container = document.getElementById('w3d-canvas-wrap')!;
    const w         = Math.max(1, container.clientWidth);
    const h         = Math.max(1, container.clientHeight);
    canvas.width    = w;
    canvas.height   = h;
    engine.resize(w, h);
  };
  window.addEventListener('resize', onResize);
  onResize();

  let prev = performance.now();
  const frame = (now: number): void => {
    const dt = (now - prev) / 1000;
    prev     = now;
    engine.tick(dt);
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
}

main().catch((err) => {
  console.error(err);
  status.textContent = `Error: ${err}`;
});
