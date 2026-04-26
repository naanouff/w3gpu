import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import type { Plugin, ResolvedConfig } from 'vite';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

/**
 * Même fichiers .glb que `examples/pbr-viewer` (`KHRONOS_GLBS` → `fixtures/phases/phase-a/glb/`)
 * + référence DamagedHelmet sous `www/public/`.
 * Servis en dev sur `/phase-a-gate/*` (fixtures) ; copiés dans `dist/phase-a-gate/` à `vite build`
 * (preview / déploiement statique sans le dossier fixtures).
 */
const FIXTURE_GLB_DIR = path.resolve(__dirname, '../fixtures/phases/phase-a/glb');
const ASSET_DIR = 'phase-a-gate';

/** Côté Vite: servis / copiés ; même six GLB que `pbr_state::KHRONOS_GLBS[1..]` (hors casque). */
export const PHASE_A_GATE_BASENAMES = [
  'AnisotropyBarnLamp.glb',
  'ClearCoatCarPaint.glb',
  'ClearcoatWicker.glb',
  'IORTestGrid.glb',
  'TextureTransformTest.glb',
  'MetalRoughSpheres.glb',
] as const;

const FIXTURE_BASENAMES_SET = new Set<string>([...PHASE_A_GATE_BASENAMES]);

function serveGlb(
  res: { statusCode: number; setHeader: (k: string, v: string) => void; end: (b: Buffer) => void },
  file: string,
) {
  try {
    const b = fs.readFileSync(file);
    res.setHeader('Content-Type', 'model/gltf-binary');
    res.setHeader('Cache-Control', 'no-cache');
    res.end(b);
  } catch {
    res.statusCode = 404;
    res.setHeader('Content-Type', 'text/plain');
    res.end(Buffer.from('not found', 'utf8'));
  }
}

function phaseAGatePlugin(): Plugin {
  let outDir: string;
  return {
    name: 'w3drs-phase-a-gate-glb',
    configResolved(c: ResolvedConfig) {
      outDir = path.resolve(c.root, c.build.outDir);
    },
    // Dev: pas de .glb copié sous public — servi depuis le dépôt fixtures. `vite build` + `preview`
    // s’appuie sur `dist/phase-a-gate/` (writeBundle), sans exiger le dossier fixtures.
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        const p = (req.url ?? '/').split('?')[0] ?? '/';
        if (!p.startsWith(`/${ASSET_DIR}/`)) {
          next();
          return;
        }
        const name = path.posix.basename(p);
        if (!name.endsWith('.glb') || !FIXTURE_BASENAMES_SET.has(name)) {
          next();
          return;
        }
        serveGlb(res, path.join(FIXTURE_GLB_DIR, name));
      });
    },
    writeBundle() {
      const to = path.join(outDir, ASSET_DIR);
      try {
        fs.mkdirSync(to, { recursive: true });
      } catch {
        // ignore
      }
      for (const name of PHASE_A_GATE_BASENAMES) {
        const from = path.join(FIXTURE_GLB_DIR, name);
        try {
          fs.copyFileSync(from, path.join(to, name));
        } catch (e) {
          const msg = e instanceof Error ? e.message : String(e);
          this.warn(
            `w3drs-phase-a-gate: copie ${name} annulée (${msg}) — vérifiez le chemin fixtures.`,
          );
        }
      }
    },
  };
}

export default phaseAGatePlugin;
