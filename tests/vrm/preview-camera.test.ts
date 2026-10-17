import assert from 'node:assert/strict';
import { test } from 'node:test';
import { Box3, Vector3 } from 'three';
import { fitBounds } from '../../packages/vrm/src/camera.ts';
import { previewCamera } from '../../packages/vrm/src/preview-camera.ts';
test('inspection view rotates and zooms without changing the export camera', () => {
  const base = fitBounds(new Box3(new Vector3(-1, 0, -1), new Vector3(1, 2, 1)), 1);
  const position = base.position.clone(), quaternion = base.quaternion.clone();
  const target = new Vector3().fromArray(base.userData.target);
  const view = previewCamera(base, { yaw: 90, pitch: 0, zoom: 2 });
  assert.ok(Math.abs(view.position.distanceTo(target) * 2 - position.distanceTo(target)) < 1e-8);
  assert.ok(view.position.x > target.x);
  assert.ok(base.position.equals(position)); assert.ok(base.quaternion.equals(quaternion));
  assert.equal(previewCamera(base, { yaw: 0, pitch: 0, zoom: 1 }), base);
  assert.throws(() => previewCamera(base, { yaw: NaN, pitch: 0, zoom: 1 }));
});
