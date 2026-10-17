import { test } from 'node:test';
import assert from 'node:assert/strict';
import { vrmClock, MAX_VRM_TIME_US } from '../../packages/vrm/src/clock.ts';
test('VRM microsecond clock preserves sub-frame times without accumulating drift', () => {
  assert.deepEqual(vrmClock(0), { tick: 0, residual: 0, seconds: 0 });
  assert.deepEqual(vrmClock(50000), { tick: 3, residual: 0, seconds: .05 });
  assert.equal(vrmClock(33333).tick, 1);
  assert.equal(vrmClock(33334).tick, 2);
  for (const time of [1, 16666, 16667, 33333, 33367, 1234567, MAX_VRM_TIME_US]) {
    const clock = vrmClock(time);
    assert.ok(clock.residual >= 0 && clock.residual < 1/60);
    assert.ok(Math.abs(clock.tick/60 + clock.residual - time/1e6) < 1e-12);
  }
  for (const time of [-1, .5, NaN, Infinity, MAX_VRM_TIME_US+1]) assert.throws(() => vrmClock(time));
});
