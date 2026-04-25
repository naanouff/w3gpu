import type { W3drsEngine } from '../../pkg/w3drs_wasm.js';

export type SceneHandles = {
  cameraEntity: number;
  modelEntities: number[];
  wallEntity: number;
  floorEntity: number;
};

const S = Math.SQRT1_2;

/**
 * Caméra, primitives GLB (même origine, pose 90° X), sol + paroi.
 */
export function buildViewerScene(
  engine: W3drsEngine,
  glbPairIds: number[],
  aspect: number,
): SceneHandles {
  const cam = engine.create_entity();
  engine.add_camera(cam, 60.0, aspect, 0.1, 200.0);
  const pitch = Math.atan2(-5, 16);
  engine.set_transform(
    cam,
    0,
    5,
    16,
    Math.sin(pitch / 2),
    0,
    0,
    Math.cos(pitch / 2),
    1,
    1,
    1,
  );

  const modelEntities: number[] = [];
  for (let i = 0; i + 1 < glbPairIds.length; i += 2) {
    const e = engine.create_entity();
    engine.set_mesh_renderer(e, glbPairIds[i]!, glbPairIds[i + 1]!);
    engine.set_transform(e, 0, 0, 0, S, 0, 0, S, 1, 1, 1);
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
