import { expect, test } from 'vitest';
import type { Clip, Project } from '@sungeki/api-client';
import { timing, selectionBounds, targets, neighbourBounds } from './timeline-gesture';
const clip: Clip = { id: 'clip', track_id: 'track', scene_id: null, start_us: 1_000_000, duration_us: 2_000_000,
  transform: { x: 0, y: 0, width: 100, height: 100, opacity: 1 }, content: { type: 'shape', shape: 'rectangle', fill: '#000000' } };
test('moving preserves length and clamps both ends to the scene', () => {
  expect(timing(clip, 'move', -2e6, 0, 5e6, [], 0, 1e6 / 30, false)).toEqual({ start_us: 0, duration_us: 2e6 });
  expect(timing(clip, 'move', 9e6, 0, 5e6, [], 0, 1e6 / 30, false)).toEqual({ start_us: 3e6, duration_us: 2e6 });
});
test('left edge preserves end and right edge preserves start without zero length', () => {
  expect(timing(clip, 'left', 5e5, 0, 5e6, [], 0, 1, false)).toEqual({ start_us: 1.5e6, duration_us: 1.5e6 });
  expect(timing(clip, 'left', 9e6, 0, 5e6, [], 0, 1, false)).toEqual({ start_us: 2999999, duration_us: 1 });
  expect(timing(clip, 'right', -9e6, 0, 5e6, [], 0, 1, false)).toEqual({ start_us: 1e6, duration_us: 1 });
});
test('moving snaps either edge; Shift bypass preserves exact microseconds', () => {
  expect(timing(clip, 'move', 980000, 0, 6e6, [4e6], 30000, 1e6 / 30, true).start_us).toBe(2e6);
  expect(timing(clip, 'move', 980000, 0, 6e6, [4e6], 30000, 1e6 / 30, false).start_us).toBe(1980000);
});
test('snap threshold scales with zoom and invalid scene targets are ignored', () => {
  expect(timing(clip, 'right', 970000, 0, 5e6, [4e6], 40000, 1, true).duration_us).toBe(3e6);
  expect(timing(clip, 'right', 970000, 0, 5e6, [4e6], 4000, 1, true).duration_us).toBe(2970000);
  expect(timing(clip, 'right', 970000, 0, 3980000, [4e6], 40000, 1, true).duration_us).toBe(2970000);
});
test('fractional FPS quantizes once without changing confirmed source', () => {
  const before = structuredClone(clip);
  expect(timing(clip, 'move', 100000, 0, 5e6, [], 0, 1e6 * 1001 / 30000, true).start_us).toBe(1101100);
  expect(clip).toEqual(before);
});

test('selection drag intersects scene bounds and excludes every selected snap edge', () => {
  const anchor = { ...clip, id: 'a', start_us: 2e6, scene_id: 's1' };
  const second = { ...clip, id: 'b', start_us: 11e6, scene_id: 's2' };
  const project = { clips: [anchor, second, { ...clip, id: 'c', start_us: 15e6 }], scenes: [
    { id: 's1', name: 'first', start_us: 0, duration_us: 10e6 },
    { id: 's2', name: 'second', start_us: 10e6, duration_us: 4e6 },
  ] } as Project;
  const limits = selectionBounds(project, ['a', 'b'], anchor);
  expect(limits).toEqual({ min: 1e6, max: 5e6 });
  expect(timing(anchor, 'move', -9e6, limits.min, limits.max, [], 0, 1, false).start_us).toBe(1e6);
  expect(timing(anchor, 'move', 9e6, limits.min, limits.max, [], 0, 1, false).start_us).toBe(3e6);
  expect(targets(project, ['a', 'b'])).toEqual([0, 10e6, 14e6, 15e6, 17e6]);
});

test('moving and trimming stop at track neighbours even without snapping', () => {
  const neighbours = [{ ...clip, id: 'before', start_us: 0, duration_us: 500000 },
    { ...clip, id: 'after', start_us: 4e6, duration_us: 1e6 },
    { ...clip, id: 'other-track', track_id: 'other', start_us: 1e6 }];
  const { min, max } = neighbourBounds(clip, neighbours);
  expect({ min, max }).toEqual({ min: 500000, max: 4e6 });
  for (const snap of [true, false]) {
    expect(timing(clip, 'move', 100e6, min, max, [], 0, 1, snap).start_us).toBe(2e6);
    expect(timing(clip, 'move', -100e6, min, max, [], 0, 1, snap).start_us).toBe(500000);
    expect(timing(clip, 'left', -100e6, min, max, [], 0, 1, snap)).toEqual({ start_us: 500000, duration_us: 2500000 });
    expect(timing(clip, 'right', 100e6, min, max, [], 0, 1, snap).duration_us).toBe(3e6);
  }
});
test('a selected neighbour moves with the selection; stationary neighbours constrain all members', () => {
  const second = { ...clip, id: 'second', start_us: 3e6 };
  const stop = { ...clip, id: 'stop', start_us: 6e6 };
  const project = { clips: [clip, second, stop], scenes: [] } as unknown as Project;
  const bounds = selectionBounds(project, ['clip', 'second'], clip);
  expect(bounds).toEqual({ min: 0, max: 4e6 });
  expect(timing(clip, 'move', 20e6, bounds.min, bounds.max, [], 0, 1, false).start_us).toBe(2e6);
});
