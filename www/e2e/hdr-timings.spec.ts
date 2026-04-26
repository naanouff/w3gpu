import { expect, test } from '@playwright/test';

/**
 * Nécessite un navigateur avec WebGPU (souvent absent en **headless** : test ignoré).
 * Pour forcer l’exécution : `npx playwright test e2e/hdr-timings.spec.ts --headed` (ou Chrome/Edge
 * `channel` pointant sur un build avec WebGPU, voir `playwright.config.ts`).
 */
test.describe('Mesures HDR (viewer www)', () => {
  test('w3drsHdrLoadTimings est défini avec des durées cohérentes', async ({ page }, testInfo) => {
    await page.goto('about:blank');
    const hasWebGPU = await page.evaluate(() => 'gpu' in navigator);
    if (!hasWebGPU) {
      testInfo.skip();
    }
    // TextureTransformTest (index 5, manifeste aligné pbr-viewer) — modèle ciblé plutôt que lourd.
    await page.goto('/?m=5', { waitUntil: 'load', timeout: 60_000 });
    await page.waitForFunction(
      () => {
        const w = window as Window & { w3drsHdrLoadTimings?: { ok?: boolean } };
        return w.w3drsHdrLoadTimings?.ok === true;
      },
      { timeout: 120_000 },
    );
    const t = await page.evaluate(() => {
      const w = window as Window & {
        w3drsHdrLoadTimings?: {
          clientBytes: number;
          clientFetchAndBufferMs: number;
          clientWasmCallWallMs: number;
          wasm: { parseMs: number; iblMs: number; envBindMs: number; totalMs: number };
        };
      };
      return w.w3drsHdrLoadTimings;
    });
    expect(t).toBeDefined();
    expect(t!.clientBytes).toBeGreaterThan(10_000);
    expect(t!.clientFetchAndBufferMs).toBeGreaterThanOrEqual(0);
    expect(t!.clientWasmCallWallMs).toBeGreaterThanOrEqual(0);
    expect(t!.wasm.totalMs).toBeGreaterThan(0);
    const sum = t!.wasm.parseMs + t!.wasm.iblMs + t!.wasm.envBindMs;
    expect(Math.abs(sum - t!.wasm.totalMs)).toBeLessThan(0.05);
  });
});
