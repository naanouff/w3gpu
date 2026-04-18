import init, { W3gpuEngine } from '../pkg/w3gpu_wasm.js';

const status = document.getElementById('status')!;

async function main(): Promise<void> {
  await init();

  status.textContent = 'Creating engine...';
  const engine = await W3gpuEngine.create('w3gpu-canvas');

  // Upload cube mesh
  const cubeMeshId = engine.upload_cube_mesh();

  // Upload PBR materials: (r, g, b, a, metallic, roughness, er, eg, eb)
  const matOrange = engine.upload_material(1.0, 0.4, 0.1, 1.0,  0.05, 0.4,  0, 0, 0);
  const matMetal  = engine.upload_material(0.8, 0.8, 0.9, 1.0,  0.9,  0.2,  0, 0, 0);
  const matGlass  = engine.upload_material(0.2, 0.7, 1.0, 1.0,  0.0,  0.05, 0, 0, 0);

  // Camera entity
  const cameraEntity = engine.create_entity();
  engine.add_camera(cameraEntity, 60.0, 800.0 / 600.0, 0.1, 1000.0);
  engine.set_transform(cameraEntity, 0, 1.5, 6,  0, 0, 0, 1,  1, 1, 1);

  // Three cubes at different positions and materials
  const cube0 = engine.create_entity();
  engine.set_mesh_renderer(cube0, cubeMeshId, matOrange);
  engine.set_transform(cube0, -1.6, 0, 0,  0, 0, 0, 1,  1, 1, 1);

  const cube1 = engine.create_entity();
  engine.set_mesh_renderer(cube1, cubeMeshId, matMetal);
  engine.set_transform(cube1, 0, 0, 0,  0, 0, 0, 1,  1, 1, 1);

  const cube2 = engine.create_entity();
  engine.set_mesh_renderer(cube2, cubeMeshId, matGlass);
  engine.set_transform(cube2, 1.6, 0, 0,  0, 0, 0, 1,  1, 1, 1);

  status.textContent = `w3gpu v${W3gpuEngine.version()} — PBR`;

  let prev = performance.now();
  let angle = 0;

  function frame(): void {
    const now = performance.now();
    const dt = (now - prev) / 1000;
    prev = now;

    angle += dt * 0.5;
    const qy = Math.sin(angle / 2);
    const qw = Math.cos(angle / 2);

    // Rotate all three cubes at slightly different speeds
    engine.set_transform(cube0, -1.6, 0, 0,  0, qy, 0, qw,  1, 1, 1);
    engine.set_transform(cube1,  0.0, 0, 0,  0, qy * 0.7, 0, Math.cos(angle * 0.35),  1, 1, 1);
    engine.set_transform(cube2,  1.6, 0, 0,  0, qy * 1.3, 0, Math.cos(angle * 0.65),  1, 1, 1);

    engine.tick(dt);
    requestAnimationFrame(frame);
  }

  requestAnimationFrame(frame);
}

main().catch((err) => {
  console.error(err);
  status.textContent = `Error: ${err}`;
});
