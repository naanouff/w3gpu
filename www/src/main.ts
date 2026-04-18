import init, { W3gpuEngine } from '../pkg/w3gpu_wasm.js';

const status = document.getElementById('status')!;

async function main(): Promise<void> {
  await init();

  status.textContent = 'Creating engine...';

  const engine = await W3gpuEngine.create('w3gpu-canvas');

  status.textContent = `w3gpu v${W3gpuEngine.version()} — running`;

  let prev = performance.now();

  function frame(): void {
    const now = performance.now();
    const dt = (now - prev) / 1000;
    prev = now;
    engine.tick(dt);
    requestAnimationFrame(frame);
  }

  requestAnimationFrame(frame);
}

main().catch((err) => {
  console.error(err);
  status.textContent = `Error: ${err}`;
});
