import type { W3drsEngine } from '../../pkg/w3drs_wasm.js';

type EngineWithReframe = W3drsEngine & {
  reframeCameraAroundModelEntities: (ids: Uint32Array) => void;
};

/**
 * Recadre l’orbite sur l’AABB des seules entités modèle (aligné sur `pbr_viewer::reframe_camera_on_scene`, sans le sol / la paroi).
 */
export function reframeOnModelEntities(
  engine: W3drsEngine,
  modelEntityIds: number[],
): void {
  (engine as EngineWithReframe).reframeCameraAroundModelEntities(
    new Uint32Array(modelEntityIds),
  );
}
