import { expect, test, vi } from 'vitest';
import { LatestPreview } from './latest-preview';
test('continuous times do not cancel completed frames; only latest pending time is decoded', async () => {
  const resolves: ((s: string) => void)[] = [];
  const load = vi.fn(() => new Promise<string>(resolve => resolves.push(resolve))), show = vi.fn();
  const queue = new LatestPreview<number>(load, show, vi.fn());
  queue.request(1); queue.request(2); queue.request(3);
  resolves.shift()!('first'); await Promise.resolve();
  expect(show).toHaveBeenCalledWith('first'); expect(load.mock.calls).toHaveLength(2);
  resolves.shift()!('last'); await Promise.resolve();
  expect(show).toHaveBeenLastCalledWith('last'); queue.dispose();
});
test('changed revision or destroyed view cannot display old image', async () => {
  let finish!: (s: string) => void;
  const show = vi.fn();
  const queue = new LatestPreview<number>(() => new Promise(resolve => finish = resolve), show, vi.fn());
  queue.request(1); queue.reset(); finish('old'); await Promise.resolve();
  expect(show).not.toHaveBeenCalled(); queue.request(2); queue.dispose(); finish('removed'); await Promise.resolve();
  expect(show).not.toHaveBeenCalled();
});
test('unchanged source across animated parent updates retains and completes images', async () => {
  const finishes: ((s: string) => void)[] = [];
  const show = vi.fn(), load = vi.fn(() => new Promise<string>(resolve => finishes.push(resolve)));
  const queue = new LatestPreview<{ time: number }>(load, show, vi.fn());
  expect(queue.select('session:revision:asset')).toBe(true);
  queue.request({ time: 0 }, 'static-image');
  for (let frame = 1; frame < 60; frame++) {
    expect(queue.select('session:revision:asset')).toBe(false);
    queue.request({ time: 0 }, 'static-image');
  }
  finishes.shift()!('background'); await Promise.resolve();
  expect(show).toHaveBeenCalledWith('background'); expect(load).toHaveBeenCalledTimes(1);
  queue.request({ time: 1 }, 'pose-1');
  expect(queue.select('session:revision:asset')).toBe(false);
  queue.request({ time: 2 }, 'pose-2');
  finishes.shift()!('first-pose'); await Promise.resolve();
  expect(show).toHaveBeenLastCalledWith('first-pose');
  expect(queue.select('session:new-revision:asset')).toBe(true);
  finishes.shift()!('obsolete-pose'); await Promise.resolve();
  expect(show).toHaveBeenLastCalledWith('first-pose'); queue.dispose();
});
