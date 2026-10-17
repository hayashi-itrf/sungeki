import { test } from 'node:test';
import assert from 'node:assert/strict';
import { BoxGeometry, Mesh, MeshBasicMaterial, Vector3 } from 'three';
import { fitCamera } from '../../packages/vrm/src/camera.ts';

test('portrait framing fits actual width and height with margin', () => {
  const geometry = new BoxGeometry(1, 3, .3);
  const material = new MeshBasicMaterial();
  const mesh = new Mesh(geometry, material);
  mesh.position.set(2, 4, -1);
  mesh.updateMatrixWorld();
  try {
    for (const aspect of [350 / 650, 650 / 680, 16 / 9]) {
      const camera = fitCamera(mesh, aspect);
      let height = 0;
      for (const x of [-.5, .5]) for (const y of [-1.5, 1.5]) for (const z of [-.15, .15]) {
        const projected = new Vector3(x + 2, y + 4, z - 1).project(camera);
        assert.ok(Math.abs(projected.x) < 1 && Math.abs(projected.y) < 1);
        assert.ok(projected.z > -1 && projected.z < 1);
        height = Math.max(height, Math.abs(projected.y));
      }
      assert.ok(height > .6, `too much empty space at aspect ${aspect}: ${height}`);
    }
    for (const aspect of [0, -1, NaN, Infinity]) assert.throws(() => fitCamera(mesh, aspect));
  } finally { geometry.dispose(); material.dispose(); }
});
