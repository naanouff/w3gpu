import init, { W3gpuEngine } from '../pkg/w3gpu_wasm.js';

const status = document.getElementById('status')!;

async function main(): Promise<void> {
  await init();

  status.textContent = 'Creating engine...';
  const engine = await W3gpuEngine.create('w3gpu-canvas');

  // Upload cube mesh, get back a GPU handle
  const cubeMeshId = engine.upload_cube_mesh();

  // Camera entity — placed at (0, 0, 5), identity rotation, looking toward origin
  const cameraEntity = engine.create_entity();
  engine.add_camera(cameraEntity, 60.0, 800.0 / 600.0, 0.1, 1000.0);
  engine.set_transform(cameraEntity, 0, 0, 5,  0, 0, 0, 1,  1, 1, 1);

  // Cube entity at origin
  const cubeEntity = engine.create_entity();
  engine.set_mesh_renderer(cubeEntity, cubeMeshId, 0);
  engine.set_transform(cubeEntity, 0, 0, 0,  0, 0, 0, 1,  1, 1, 1);

  status.textContent = `w3gpu v${W3gpuEngine.version()} — running`;

  let prev = performance.now();
  let angle = 0;

  function frame(): void {
    const now = performance.now();
    const dt = (now - prev) / 1000;
    prev = now;

    // Rotate cube around Y axis
    angle += dt * 0.8;
    const qy = Math.sin(angle / 2);
    const qw = Math.cos(angle / 2);
    engine.set_transform(cubeEntity, 0, 0, 0,  0, qy, 0, qw,  1, 1, 1);

    engine.tick(dt);
    requestAnimationFrame(frame);
  }

  requestAnimationFrame(frame);
}

main().catch((err) => {
  console.error(err);
  status.textContent = `Error: ${err}`;
});
