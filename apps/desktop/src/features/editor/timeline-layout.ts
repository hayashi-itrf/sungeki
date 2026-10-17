import type { Clip, Track } from '@sungeki/api-client';
export const LANE_HEIGHT = 34;
export const TRACK_GAP = 8;
export interface Cell { clip: Clip; lane: number }
export interface TrackRow { track: Track; top: number; height: number; cells: Cell[] }
interface Slot { end: number; lane: number }
function earlier(a: Slot, b: Slot) { return a.end < b.end || (a.end === b.end && a.lane < b.lane); }
function push(heap: Slot[], item: Slot) {
  heap.push(item);
  let i = heap.length - 1;
  while (i > 0) {
    const parent = (i - 1) >> 1;
    if (!earlier(heap[i], heap[parent])) break;
    [heap[i], heap[parent]] = [heap[parent], heap[i]]; i = parent;
  }
}
function pop(heap: Slot[]): Slot {
  const first = heap[0]; const last = heap.pop()!;
  if (heap.length) {
    heap[0] = last; let i = 0;
    while (i * 2 + 1 < heap.length) {
      let child = i * 2 + 1;
      if (child + 1 < heap.length && earlier(heap[child + 1], heap[child])) child++;
      if (!earlier(heap[child], heap[i])) break;
      [heap[i], heap[child]] = [heap[child], heap[i]]; i = child;
    }
  }
  return first;
}
export function layoutTracks(tracks: Track[], clips: Clip[], collapsed: Set<string>): TrackRow[] {
  const byTrack = new Map<string, Clip[]>();
  for (const clip of clips) {
    if (clip.scene_id && collapsed.has(clip.scene_id)) continue;
    const list = byTrack.get(clip.track_id) ?? [];
    list.push(clip); byTrack.set(clip.track_id, list);
  }
  let top = 0;
  return tracks.map(track => {
    const heap: Slot[] = []; let lanes = 0;
    // Stable sorting preserves the source order for equal starts; Project is never mutated.
    const cells = (byTrack.get(track.id) ?? []).slice().sort((a, b) => a.start_us - b.start_us).map(clip => {
      const lane = heap.length && heap[0].end <= clip.start_us ? pop(heap).lane : lanes++;
      push(heap, { lane, end: clip.start_us + clip.duration_us });
      return { clip, lane };
    });
    const height = Math.max(1, lanes) * LANE_HEIGHT;
    const row = { track, top, height, cells }; top += height + TRACK_GAP;
    return row;
  });
}
export function visibleCells(row: TrackRow, start: number, end: number, top: number, height: number): Cell[] {
  return row.cells.filter(({ clip, lane }) => clip.start_us < end * 1_000_000
    && clip.start_us + clip.duration_us > start * 1_000_000
    && row.top + (lane + 1) * LANE_HEIGHT > top - LANE_HEIGHT
    && row.top + lane * LANE_HEIGHT < top + height + LANE_HEIGHT);
}
export function clipBounds(clip: Clip, start: number, span: number) {
  const left = Math.max(start, clip.start_us / 1_000_000);
  const end = Math.min(start + span, (clip.start_us + clip.duration_us) / 1_000_000);
  return { left: 100 * (left - start) / span, width: 100 * Math.max(0, end - left) / span };
}
export function timeLabel(time: number) {
  const rounded = Math.round(time * 100);
  const minutes = Math.floor(rounded / 6000); const seconds = (rounded % 6000) / 100;
  return `${minutes}:${seconds.toFixed(2).padStart(5, '0')}`;
}
