import type { Clip, Project } from '@sungeki/api-client';
export type GestureMode = 'move' | 'left' | 'right';
export interface Timing { start_us: number; duration_us: number }
const MAX = 604_800_000_000;
export function targets(project: Project, clipId: string | string[]): number[] {
  const excluded = new Set(typeof clipId === 'string' ? [clipId] : clipId);
  return [...new Set([0, ...project.clips.filter(c => !excluded.has(c.id)).flatMap(c => [c.start_us, c.start_us + c.duration_us]),
    ...project.scenes.flatMap(s => [s.start_us, s.start_us + s.duration_us])])].sort((a, b) => a - b);
}
function nearest(values: number[], value: number): number {
  let lo = 0; let hi = values.length;
  while (lo < hi) { const mid = (lo + hi) >>> 1; if (values[mid] < value) lo = mid + 1; else hi = mid; }
  const a = values[lo - 1] ?? Infinity; const b = values[lo] ?? Infinity;
  return Math.abs(a - value) <= Math.abs(b - value) ? a : b;
}
export function timing(clip: Clip, mode: GestureMode, deltaUs: number, min: number, max: number,
  snapTargets: number[], threshold: number, frameUs: number, snap: boolean): Timing {
  max = Math.min(max, MAX); min = Math.max(0, min);
  const end = clip.start_us + clip.duration_us;
  let value = (mode === 'right' ? end : clip.start_us) + deltaUs;
  const low = mode === 'right' ? clip.start_us + 1 : min;
  const high = mode === 'left' ? end - 1 : mode === 'move' ? max - clip.duration_us : max;
  const clamp = (n: number) => Math.max(low, Math.min(high, Math.round(n)));
  value = clamp(value);
  if (snap) {
    const offsets = mode === 'move' ? [0, clip.duration_us] : [0];
    let candidate = value; let distance = Infinity;
    for (const offset of offsets) {
      const target = nearest(snapTargets, value + offset) - offset;
      const d = Math.abs(target - value);
      if (target >= low && target <= high && d <= threshold && d < distance) { candidate = target; distance = d; }
    }
    value = distance < Infinity ? candidate : clamp(Math.round(value / frameUs) * frameUs);
  }
  value = clamp(value);
  return mode === 'move' ? { start_us: value, duration_us: clip.duration_us }
    : mode === 'left' ? { start_us: value, duration_us: end - value }
    : { start_us: clip.start_us, duration_us: value - clip.start_us };
}

export function selectionBounds(project: Project, ids: string[], anchor: Clip): { min: number; max: number } {
  const selected = new Set(ids);
  const scenes = new Map(project.scenes.map(s => [s.id, s]));
  const byTrack = new Map<string, Clip[]>();
  for (const clip of project.clips) {
    if (selected.has(clip.id)) continue;
    const list = byTrack.get(clip.track_id) ?? [];
    list.push(clip); byTrack.set(clip.track_id, list);
  }
  let low = -MAX; let high = MAX;
  for (const clip of project.clips) {
    if (!selected.has(clip.id)) continue;
    const scene = clip.scene_id ? scenes.get(clip.scene_id) : undefined;
    const bounds = neighbourBounds(clip, byTrack.get(clip.track_id) ?? []);
    low = Math.max(low, bounds.min - clip.start_us);
    high = Math.min(high, bounds.max - clip.start_us - clip.duration_us);
    low = Math.max(low, (scene?.start_us ?? 0) - clip.start_us);
    high = Math.min(high, (scene ? scene.start_us + scene.duration_us : MAX) - clip.start_us - clip.duration_us);
  }
  return { min: anchor.start_us + low, max: anchor.start_us + anchor.duration_us + high };
}

/** Gesture limits only: confirmed edits still go through the shared host. */
export function neighbourBounds(clip: Clip, neighbours: Clip[]): { min: number; max: number } {
  let min = 0; let max = MAX;
  const end = clip.start_us + clip.duration_us;
  for (const other of neighbours) {
    if (other.id === clip.id || other.track_id !== clip.track_id) continue;
    const otherEnd = other.start_us + other.duration_us;
    if (otherEnd <= clip.start_us) min = Math.max(min, otherEnd);
    else if (other.start_us >= end) max = Math.min(max, other.start_us);
    else { min = Math.max(min, clip.start_us); max = Math.min(max, end); }
  }
  return { min, max };
}
