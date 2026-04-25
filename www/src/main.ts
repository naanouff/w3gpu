import init, { W3drsEngine, w3drsValidateRenderGraphV0 } from '../pkg/w3drs_wasm.js';
import { loadHdrWithTimings } from './hdrLoadTimings.js';

const status = document.getElementById('status')!;

const GRID    = 5;
const SPACING = 2.4;
// base_x(+90°) = (qx=√½, qy=0, qz=0, qw=√½)
const S = Math.SQRT1_2;

let cullEnabled = true;

type ViewerModel = { id: string; url: string };
type ViewerManifest = { version: number; models: ViewerModel[] };

const FALLBACK_URL = '/damaged_helmet_source_glb.glb';
const FALLBACK_ID  = 'damaged_helmet_gate';

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

function goAdjacentModel(nModels: number, delta: number): void {
  if (nModels < 2) {
    return;
  }
  const cur   = readModelIndexOrZero(nModels);
  const next  = (cur + delta + nModels) % nModels;
  const u     = new URL(window.location.href);
  u.searchParams.set('m', String(next));
  window.location.assign(u.toString());
}

async function main(): Promise<void> {
  await init();

  let phaseBJsonText: string | null = null;
  // Phase B — parse + validate_exec_v0 (sans GPU) ; B.5 : encodage GPU + checksum (après moteur créé)
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
      const sum = engine.w3drsPhaseBGraphRunChecksum(phaseBJsonText, JSON.stringify(wgsl), 'hdr_color') as string;
      console.info(`[w3drs] Phase B.5: GPU graph checksum = ${String(sum)} (w3drsPhaseBGraphRunChecksum, same encode as native)`);
    } catch (e) {
      console.warn('[w3drs] Phase B.5: GPU run skipped (optional):', e);
    }
  }

  try {
    const cfg = await fetch('/phase-a/materials/default.json');
    if (cfg.ok) {
      engine.applyPhaseAViewerConfigJson(await cfg.text());
    }
  } catch (e) {
    console.warn('Phase A viewer config not applied (optional):', e);
  }

  // IBL — mesures : tests fonctionnels dans `hdrLoadTimings.test.ts` ; E2E `e2e/hdr-timings.spec.ts`
  status.textContent = 'Loading environment...';
  try {
    const hdrResult = await loadHdrWithTimings(
      engine,
      '/studio_small_03_2k.hdr',
    );
    if (hdrResult.ok) {
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

  // GLB — aligné sur fixtures/…/manifest (ids) via viewer-manifest.json (URL publiques)
  let modelUrl  = FALLBACK_URL;
  let modelId   = FALLBACK_ID;
  let nModels   = 1;
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

  status.textContent = 'Loading model...';
  const response = await fetch(modelUrl);
  if (!response.ok) {
    throw new Error(`Model HTTP ${response.status} (${modelId})`);
  }
  const ids = engine.load_gltf(new Uint8Array(await response.arrayBuffer()));
  if (ids.length < 2) {
    throw new Error('No primitives in GLB');
  }

  const meshId = ids[0]!;
  const matId  = ids[1]!;

  // Camera
  const cam = engine.create_entity();
  engine.add_camera(cam, 60.0, window.innerWidth / window.innerHeight, 0.1, 200.0);
  // look_at_rh(eye=(0,5,16), target=(0,0,0)): pitch ≈ atan2(-5,16)
  const pitch = Math.atan2(-5, 16);
  engine.set_transform(cam,
    0, 5, 16,
    Math.sin(pitch / 2), 0, 0, Math.cos(pitch / 2),
    1, 1, 1,
  );

  // 5×5 grid — all same mesh → batching
  const entities = new Array<number>();
  const phases   = new Array<number>();
  for (let row = 0; row < GRID; row++) {
    for (let col = 0; col < GRID; col++) {
      const x     = (col - Math.floor(GRID / 2)) * SPACING;
      const z     = (row - Math.floor(GRID / 2)) * SPACING;
      const phase = (row * GRID + col) * (Math.PI * 2 / (GRID * GRID));
      const e     = engine.create_entity();
      engine.set_mesh_renderer(e, meshId, matId);
      engine.set_transform(e, x, 0, z,  S, 0, 0, S,  1, 1, 1);
      entities.push(e);
      phases.push(phase);
    }
  }

  // Occluder wall
  const wallMesh = engine.upload_cube_mesh();
  const wallMat  = engine.upload_material(0.8, 0.05, 0.05, 1.0, 0.9, 0.2, 0, 0, 0);
  const wall     = engine.create_entity();
  engine.set_mesh_renderer(wall, wallMesh, wallMat);
  engine.set_transform(wall, 0, 0.8, -1.2,  0, 0, 0, 1,  7, 3, 0.25);

  // Ground
  const floorMesh = engine.upload_cube_mesh();
  const floorMat  = engine.upload_material(0.35, 0.35, 0.35, 1.0, 0.0, 0.9, 0, 0, 0);
  const floor     = engine.create_entity();
  engine.set_mesh_renderer(floor, floorMesh, floorMat);
  engine.set_transform(floor, 0, -1.2, 0,  0, 0, 0, 1,  14, 0.05, 14);

  const modelHint = nModels > 1
    ? `  ${modelId}  [m=${String(modelIndex + 1)}/${String(nModels)}]  ←/→  `
    : `  ${modelId}  `;

  const updateStatus = (): void => {
    const c = cullEnabled ? 'ON' : 'OFF';
    status.textContent =
      `w3drs v${W3drsEngine.version()}` + modelHint +
      `— GPU Hi-Z: ${c}  [SPACE]  `;
  };
  updateStatus();

  document.addEventListener('keydown', (e) => {
    if (e.code === 'Space') {
      cullEnabled = !cullEnabled;
      engine.set_cull_enabled(cullEnabled);
      updateStatus();
      e.preventDefault();
      return;
    }
    if (e.code === 'ArrowLeft') {
      goAdjacentModel(nModels, -1);
      e.preventDefault();
      return;
    }
    if (e.code === 'ArrowRight') {
      goAdjacentModel(nModels, 1);
      e.preventDefault();
    }
  });

  let prev      = performance.now();
  let totalTime = 0;

  function frame(): void {
    const now = performance.now();
    const dt  = (now - prev) / 1000;
    prev = now;
    totalTime += dt;

    for (let i = 0; i < entities.length; i++) {
      const angle = totalTime * 0.4 + phases[i]!;
      const ha  = angle / 2;
      const qx  =  S * Math.cos(ha);
      const qy  =  S * Math.sin(ha);
      const qz  = -S * Math.sin(ha);
      const qw  =  S * Math.cos(ha);
      const x   = ((i % GRID) - Math.floor(GRID / 2)) * SPACING;
      const z   = (Math.floor(i / GRID) - Math.floor(GRID / 2)) * SPACING;
      engine.set_transform(entities[i]!, x, 0, z,  qx, qy, qz, qw,  1, 1, 1);
    }

    engine.tick(dt);
    requestAnimationFrame(frame);
  }

  requestAnimationFrame(frame);
}

main().catch((err) => {
  console.error(err);
  status.textContent = `Error: ${err}`;
});
