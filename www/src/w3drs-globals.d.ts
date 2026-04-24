import type { HdrLoadTimingsOk } from './hdrLoadTimings.js';

declare global {
  interface Window {
    /** Dernière mesure HDR réussie (ex. tests E2E / debug). */
    w3drsHdrLoadTimings?: HdrLoadTimingsOk;
  }
}

export {};
