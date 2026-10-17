import { test } from 'node:test';
import assert from 'node:assert/strict';
import { BoxGeometry, Mesh, MeshBasicMaterial, Group, Vector3 } from 'three';
import { motionBounds } from '../../packages/vrm/src/motion-bounds.ts';
import { fitBounds } from '../../packages/vrm/src/camera.ts';

test('motion framing includes translation beyond the first pose with a bounded sample count', () => {
  const geometry = new BoxGeometry(1, 2, .2), material = new MeshBasicMaterial();
  const mesh = new Mesh(geometry, material), scene = new Group(); scene.add(mesh);
  let samples = 0;
  const avatar = { duration: 60, mixer: { setTime(t: number) { samples++; mesh.position.x = 4 * Math.sin(t * Math.PI / 60); } },
    vrm: { scene, update() {} } } as unknown as Parameters<typeof motionBounds>[0];
  try {
    const bounds = motionBounds(avatar);
    assert.equal(samples, 601);
    assert.ok(bounds.min.x <= -.5 && bounds.max.x >= 4.5);
    const camera = fitBounds(bounds, 360 / 640);
    for (const x of [-.5, 4.5]) for (const y of [-1, 1]) {
      const p = new Vector3(x, y, .1).project(camera);
      assert.ok(Math.abs(p.x) < 1 && Math.abs(p.y) < 1);
    }
  } finally { geometry.dispose(); material.dispose(); }
});
