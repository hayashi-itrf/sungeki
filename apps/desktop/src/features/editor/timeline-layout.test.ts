import { expect, test } from 'vitest';
import type { Clip, Track } from '@sungeki/api-client';
import { clipBounds, layoutTracks, LANE_HEIGHT, timeLabel, visibleCells } from './timeline-layout';
const track = (id: string): Track => ({ id, name: id, kind: 'visual', visible: true, muted: false });
const clip = (id: string, start: number, duration: number, tid = 't'): Clip => ({ id, track_id: tid,
  scene_id: null, start_us: start * 1_000_000, duration_us: duration * 1_000_000,
  transform: { x: 0, y: 0, width: 100, height: 100, opacity: 1 },
  content: { type: 'text', text: id, font_size: 20, color: '#000000' } });

test('overlap creates separate selectable lanes; touching clips reuse a lane without mutating source', () => {
  const source = [clip('later', 5, 2), clip('a', 0, 5), clip('b', 0, 10), clip('c', 3, 2)];
  const saved = structuredClone(source);
  const [row] = layoutTracks([track('t')], source, new Set());
  expect(source).toEqual(saved);
  expect(row.cells.map(c => [c.clip.id, c.lane])).toEqual([['a', 0], ['b', 1], ['c', 2], ['later', 0]]);
  expect(row.height).toBe(3 * LANE_HEIGHT);
});

test('interval heap never places overlapping clips in the same lane', () => {
  const source = Array.from({ length: 1000 }, (_, i) => clip(String(i), (i * 7919) % 101, (i % 9) + 1));
  const [row] = layoutTracks([track('t')], source, new Set());
  const ends = new Map<number, number>();
  for (const cell of row.cells) {
    expect(cell.clip.start_us).toBeGreaterThanOrEqual(ends.get(cell.lane) ?? 0);
    ends.set(cell.lane, cell.clip.start_us + cell.clip.duration_us);
  }
});

test('half-open viewport includes crossing clips and excludes exact outside boundaries', () => {
  const source = [clip('crossing', 0, 20), clip('before', 1, 9), clip('inside', 12, 1), clip('after', 15, 2)];
  const [row] = layoutTracks([track('t')], source, new Set());
  expect(visibleCells(row, 10, 15, 0, 1000).map(c => c.clip.id)).toEqual(['crossing', 'inside']);
  expect(clipBounds(source[0], 10, 5)).toEqual({ left: 0, width: 100 });
});

test('collapsed scenes preserve other clips and track order', () => {
  const a = clip('a', 0, 5, 'second'); a.scene_id = 'scene';
  const rows = layoutTracks([track('first'), track('second')], [a, clip('b', 1, 2, 'second')], new Set(['scene']));
  expect(rows.map(r => r.track.id)).toEqual(['first', 'second']);
  expect(rows[1].cells.map(c => c.clip.id)).toEqual(['b']);
});

test('5000 clips / 50 tracks restrict render candidates by both time and lane viewport', () => {
  const tracks = Array.from({ length: 50 }, (_, i) => track(String(i)));
  const clips = Array.from({ length: 5000 }, (_, i) => clip(String(i), Math.floor(i / 50), 1, String(i % 50)));
  const rows = layoutTracks(tracks, clips, new Set());
  const shown = rows.flatMap(r => visibleCells(r, 40, 50, 400, 150));
  expect(shown.length).toBeGreaterThan(0);
  expect(shown.length).toBeLessThan(100);
  expect(shown.every(c => c.clip.start_us >= 40_000_000 && c.clip.start_us < 50_000_000)).toBe(true);
  const [overlapped] = layoutTracks([track('t')], Array.from({ length: 5000 }, (_, i) => clip(String(i), 0, 1)), new Set());
  expect(visibleCells(overlapped, 0, 1, 1000, 150).length).toBeLessThan(10);
});

test('time labels keep subsecond positions', () => {
  expect(timeLabel(12.4)).toBe('0:12.40'); expect(timeLabel(65)).toBe('1:05.00'); expect(timeLabel(59.999)).toBe('1:00.00');
});
