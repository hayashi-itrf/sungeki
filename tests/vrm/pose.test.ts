import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Object3D } from 'three';
import { applyPose, validatePose } from '../../packages/vrm/src/pose.ts';
import { parseRenderRequest } from '../../workers/vrm/src/render-request.ts';
test('normalized joint rotation uses XYZ degrees and rejects unknown or nonfinite inputs', () => {
  const node = new Object3D();
  const avatar = { humanoid: { getNormalizedBoneNode: (name: string) => name === 'leftUpperArm' ? node : null } } as unknown as Parameters<typeof applyPose>[0];
  applyPose(avatar, { leftUpperArm: [0, 0, 90] });
  assert.ok(Math.abs(node.quaternion.z - Math.SQRT1_2) < 1e-12);
  assert.throws(() => applyPose(avatar, { head: [0,0,0] }));
  for (const pose of [{ unknown:[0,0,0] }, { head:[0,NaN,0] }, { head:[0,181,0] }]) assert.throws(() => validatePose(pose as never));
});
test('render requests carry pose overrides and reject invalid pose data', () => {
  const source = { path:'/tmp/model', sha256:'a'.repeat(64), byte_length:20 };
  const request = { version:1, model:source, motion:source, width:320, height:180,
    frames:[{ time_us:0, expressions:{}, pose:{head:[10,0,0]} }] };
  assert.deepEqual(parseRenderRequest(request).frames[0].pose, {head:[10,0,0]});
  request.frames[0].pose.head = [Infinity,0,0];
  assert.throws(() => parseRenderRequest(request));
});
