import { expect, test } from 'vitest';
import { nextVolumePoint } from './volume-points';

test('new points extend the held ends and preserve existing interpolation', () => {
  expect(nextVolumePoint([], 1e6)).toEqual({ at_us: 0, level: 1 });
  expect(nextVolumePoint([{ at_us: 0, level: .4 }], 1e6)).toEqual({ at_us: 1e6, level: .4 });
  expect(nextVolumePoint([{ at_us: 50, level: .4 }], 100)).toEqual({ at_us: 0, level: .4 });
  const points = Object.freeze([Object.freeze({ at_us: 0, level: 1 }), Object.freeze({ at_us: 101, level: 0 })]);
  const added = nextVolumePoint(points, 101)!;
  expect(added.at_us).toBe(50);
  expect(added.level).toBeCloseTo(1 - 50 / 101, 14);
  expect(points).toHaveLength(2);
});
test('only distinct legal microseconds are offered within the maximum point count', () => {
  expect(nextVolumePoint([{ at_us: 0, level: 1 }, { at_us: 1, level: 0 }], 1)).toBeNull();
  expect(nextVolumePoint(Array.from({ length: 64 }, (_, i) => ({ at_us: i, level: 1 })), 100)).toBeNull();
  for (const points of [[{at_us: NaN, level: 1}], [{at_us: 2, level: 1}, {at_us: 1, level: 1}],
    [{at_us: 11, level: 1}], [{at_us: 0, level: NaN}]]) expect(nextVolumePoint(points, 10)).toBeNull();
  expect(nextVolumePoint([], NaN)).toBeNull();
});
