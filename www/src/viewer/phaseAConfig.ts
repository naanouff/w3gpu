/** Paramètres Phase A alignés sur `w3drs-assets` / `default.json`. */

const VARIANT = 'live';

export type LiveTonemap = {
  exposure: number;
  bloom_strength: number;
  /** Post-tonemap FXAA (défaut aligné Rust `PhaseATonemap::fxaa`). */
  fxaa: boolean;
};

export type LivePhaseA = {
  ibl_diffuse_scale: number;
  ibl_tier: string;
  tonemap: LiveTonemap;
};

export const DEFAULT_LIVE: LivePhaseA = {
  ibl_diffuse_scale: 1.0,
  ibl_tier: 'min',
  tonemap: { exposure: 1.0, bloom_strength: 0.0, fxaa: true },
};

/** JSON pour `applyPhaseAViewerConfigJson` — une variante `live` pour ne pas mélanger avec d’éventuels fixtures chargés. */
export function toPhaseAJson(live: LivePhaseA, activeVariant: string = VARIANT): string {
  return JSON.stringify({
    version: 1,
    active_variant: activeVariant,
    comment: 'viewer — variante générée côté client',
    variants: {
      [activeVariant]: {
        label: 'Viewer (live)',
        ibl_tier: live.ibl_tier,
        ibl_diffuse_scale: live.ibl_diffuse_scale,
        tonemap: {
          exposure: live.tonemap.exposure,
          bloom_strength: live.tonemap.bloom_strength,
          fxaa: live.tonemap.fxaa,
        },
      },
    },
  });
}
