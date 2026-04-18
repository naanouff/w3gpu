import init, { W3gpuEngine } from '../pkg/w3gpu_wasm.js';

const status = document.getElementById('status')!;

async function main(): Promise<void> {
  await init();

  status.textContent = 'Creating engine...';
  const engine = await W3gpuEngine.create('w3gpu-canvas');

  // Load GLB from public folder
  status.textContent = 'Loading model...';
  const response = await fetch('/damaged_helmet_source_glb.glb');
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  const bytes = new Uint8Array(await response.arrayBuffer());

  // Returns flat array [mesh_id0, mat_id0, mesh_id1, mat_id1, ...]
  const ids = engine.load_gltf(bytes);
  if (ids.length < 2) throw new Error('No primitives found in GLB');

  // Camera
  const cameraEntity = engine.create_entity();
  engine.add_camera(cameraEntity, 60.0, 800.0 / 600.0, 0.1, 1000.0);
  engine.set_transform(cameraEntity, 0, 0, 3,  0, 0, 0, 1,  1, 1, 1);

  // Spawn one entity per primitive
  const meshEntities = new Array<number>();
  for (let i = 0; i + 1 < ids.length; i += 2) {
    const meshId = ids[i];
    const matId  = ids[i + 1];
    const entity = engine.create_entity();
    engine.set_mesh_renderer(entity, meshId, matId);
    engine.set_transform(entity, 0, 0, 0,  0, 0, 0, 1,  1, 1, 1);
    meshEntities.push(entity);
  }

  status.textContent = `w3gpu v${W3gpuEngine.version()} — ${meshEntities.length} primitives`;

  let prev = performance.now();
  let angle = 0;

  function frame(): void {
    const now = performance.now();
    const dt = (now - prev) / 1000;
    prev = now;

    angle += dt * 0.4;
    const qy = Math.sin(angle / 2);
    const qw = Math.cos(angle / 2);

    for (const entity of meshEntities) {
      engine.set_transform(entity, 0, 0, 0,  0, qy, 0, qw,  1, 1, 1);
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
