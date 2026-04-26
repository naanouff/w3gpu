import { defineConfig } from 'vite';
import phaseAGatePlugin from './vite-plugin-phase-a-gate';

export default defineConfig({
  plugins: [phaseAGatePlugin()],
  server: {
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
