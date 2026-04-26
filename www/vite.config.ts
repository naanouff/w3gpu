import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import phaseAGatePlugin from './vite-plugin-phase-a-gate';

const __d = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [phaseAGatePlugin()],
  server: {
    fs: {
      allow: [path.join(__d, '..')],
    },
    headers: {
      // Required for SharedArrayBuffer (WASM threads) and WebGPU
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'require-corp',
    },
  },
  build: {
    target: 'esnext',
  },
});
