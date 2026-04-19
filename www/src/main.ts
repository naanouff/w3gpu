import init, { W3gpuEngine } from '../pkg/w3gpu_wasm.js';

const status = document.getElementById('status')!;

const GRID    = 5;
const SPACING = 2.4;
// base_x(+90°) = (qx=√½, qy=0, qz=0, qw=√½)
const S = Math.SQRT1_2;

async function main(): Promise<void> {
  await init();

  status.textContent = 'Creating engine...';
  const engine = await W3gpuEngine.create('w3gpu-canvas');

  // IBL
  status.textContent = 'Loading environment...';
  try {
    const hdrResponse = await fetch('/studio_small_03_2k.hdr');
    if (hdrResponse.ok) {
      engine.load_hdr(new Uint8Array(await hdrResponse.arrayBuffer()));
    }
  } catch (e) {
    console.warn('HDR load failed, using default IBL:', e);
  }

  // GLB
  status.textContent = 'Loading model...';
  const response = await fetch('/damaged_helmet_source_glb.glb');
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  const ids = engine.load_gltf(new Uint8Array(await response.arrayBuffer()));
  if (ids.length < 2) throw new Error('No primitives in GLB');

  const meshId = ids[0];
  const matId  = ids[1];

  // Camera — framed for the full 5×5 grid
  const cam = engine.create_entity();
  engine.add_camera(cam, 60.0, window.innerWidth / window.innerHeight, 0.1, 200.0);
  // Position: (0, 5, 16) looking at origin → quaternion from look_at
  // We approximate: tilt slightly down, no lateral spin
  // pitch = atan2(-5, 16) ≈ -17.4° → half-angle ≈ -8.7°
  const pitch = Math.atan2(-5, 16);
  const cpx   = Math.sin(pitch / 2);
  const cpw   = Math.cos(pitch / 2);
  engine.set_transform(cam, 0, 5, 16,  cpx, 0, 0, cpw,  1, 1, 1);

  // 5×5 helmet grid — all same (meshId, matId) → 1 draw call via batching
  const entities = new Array<number>();
  const phases   = new Array<number>();
  for (let row = 0; row < GRID; row++) {
    for (let col = 0; col < GRID; col++) {
      const x     = (col - Math.floor(GRID / 2)) * SPACING;
      const z     = (row - Math.floor(GRID / 2)) * SPACING;
      const phase = (row * GRID + col) * (Math.PI * 2 / (GRID * GRID));
      const entity = engine.create_entity();
      engine.set_mesh_renderer(entity, meshId, matId);
      engine.set_transform(entity, x, 0, z,  S, 0, 0, S,  1, 1, 1);
      entities.push(entity);
      phases.push(phase);
    }
  }

  // Ground plane — different material → separate batch (2nd draw call)
  const floorMesh = engine.upload_cube_mesh();
  const floorMat  = engine.upload_material(0.35, 0.35, 0.35, 1.0, 0.0, 0.9, 0, 0, 0);
  const floor = engine.create_entity();
  engine.set_mesh_renderer(floor, floorMesh, floorMat);
  engine.set_transform(floor, 0, -1.2, 0,  0, 0, 0, 1,  14, 0.05, 14);

  const instanceCount = entities.length;
  const batchCount    = 2; // helmets + floor
  status.textContent =
    `w3gpu v${W3gpuEngine.version()} — ${instanceCount} instances → ${batchCount} batches → ${batchCount} draw calls [indirect]`;

  let prev      = performance.now();
  let totalTime = 0;

  function frame(): void {
    const now = performance.now();
    const dt  = (now - prev) / 1000;
    prev = now;
    totalTime += dt;

    // Per-entity staggered Y-spin: y_spin(angle+phase) * base_x(+90°)
    for (let i = 0; i < entities.length; i++) {
      const angle = totalTime * 0.4 + phases[i];
      const ha  = angle / 2;
      const qx  =  S * Math.cos(ha);
      const qy  =  S * Math.sin(ha);
      const qz  = -S * Math.sin(ha);
      const qw  =  S * Math.cos(ha);
      const x   = (( i % GRID) - Math.floor(GRID / 2)) * SPACING;
      const z   = (Math.floor(i / GRID) - Math.floor(GRID / 2)) * SPACING;
      engine.set_transform(entities[i], x, 0, z,  qx, qy, qz, qw,  1, 1, 1);
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
