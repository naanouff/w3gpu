import { defineConfig, devices } from '@playwright/test';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

/**
 * E2E réel (WebGPU + WASM). `npm run test` = Vitest uniquement (sans navigateur).
 * Première exécution : `npx playwright install chromium`.
 */
export default defineConfig({
  testDir: path.join(__dirname, 'e2e'),
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  use: {
    ...devices['Desktop Chrome'],
  },
  webServer: {
    command: 'npm run build:wasm && npx vite --port 5173 --strictPort',
    cwd: __dirname,
    url: 'http://localhost:5173',
    reuseExistingServer: !process.env.CI,
    timeout: 180_000,
  },
});
