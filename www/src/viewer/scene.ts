import type { W3drsEngine } from '../../pkg/w3drs_wasm.js';

export type SceneHandles = {
  cameraEntity: number;
  modelEntities: number[];
  wallEntity: number;
  floorEntity: number;
};

/**
 * Même scène d’esprit que le viewer natif : caméra + orbite 6/0,22/0
 * (sans forcer de pose manuelle : le premier `tick` / `reframe` alignent l’Œil),
 * primitives GLB avec `TransformComponent` **identité** (comme `pbr_state::upload_primitives`),
 * sol + paroi décoratifs (hors AABB de recadrage, voir `reframeOnModelEntities` dans `main`).
 */
export function buildViewerScene(
  engine: W3drsEngine,
  glbPairIds: number[],
  aspect: number,
): SceneHandles {
  const cam = engine.create_entity();
  // `pbr_state::new` : 60° FOV, near 0.1, far 300, transform caméra par défaut (identité) puis orbit.
  engine.set_transform(cam, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1);
  engine.add_camera(cam, 60.0, aspect, 0.1, 300.0);
  // Pas de `w3drsSyncOrbitFromCamera` ici : l’`OrbitController` post-`clearSceneForNewGltf`
  // est déjà `new(6, 0.22, 0, ZERO)` comme le natif.

  const modelEntities: number[] = [];
  for (let i = 0; i + 1 < glbPairIds.length; i += 2) {
    const e = engine.create_entity();
    engine.set_mesh_renderer(e, glbPairIds[i]!, glbPairIds[i + 1]!);
    engine.set_transform(e, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1);
    modelEntities.push(e);
  }

  const wallMesh = engine.upload_cube_mesh();
  const wallMat = engine.upload_material(0.8, 0.05, 0.05, 1.0, 0.9, 0.2, 0, 0, 0);
  const wall = engine.create_entity();
  engine.set_mesh_renderer(wall, wallMesh, wallMat);
  engine.set_transform(wall, 0, 0.8, -1.2, 0, 0, 0, 1, 7, 3, 0.25);

  const floorMesh = engine.upload_cube_mesh();
  const floorMat = engine.upload_material(0.35, 0.35, 0.35, 1.0, 0.0, 0.9, 0, 0, 0);
  const floor = engine.create_entity();
  engine.set_mesh_renderer(floor, floorMesh, floorMat);
  engine.set_transform(floor, 0, -1.2, 0, 0, 0, 0, 1, 14, 0.05, 14);

  return { cameraEntity: cam, modelEntities, wallEntity: wall, floorEntity: floor };
}
