import type { VolumePoint } from '@sungeki/api-client';

// Choose an unused clip-relative microsecond without changing existing points.
export function nextVolumePoint(points: readonly VolumePoint[], durationUs: number): VolumePoint | null {
  if (points.length >= 64 || !Number.isSafeInteger(durationUs) || durationUs < 1) return null;
  if (!points.length) return { at_us: 0, level: 1 };
  if (points.some((p, i) => !Number.isSafeInteger(p.at_us) || p.at_us < 0 || p.at_us > durationUs
    || !Number.isFinite(p.level) || p.level < 0 || p.level > 1 || (i > 0 && p.at_us <= points[i - 1].at_us))) return null;
  if (points[0].at_us > 0) return { at_us: 0, level: points[0].level };
  const last = points[points.length - 1];
  if (last.at_us < durationUs) return { at_us: durationUs, level: last.level };
  let gap = 1, chosen: VolumePoint | null = null;
  for (let i = 1; i < points.length; i++) {
    const a = points[i - 1], b = points[i];
    if (b.at_us - a.at_us > gap) {
      gap = b.at_us - a.at_us;
      const at_us = a.at_us + Math.floor(gap / 2);
      chosen = { at_us, level: a.level + (b.level - a.level) * (at_us - a.at_us) / gap };
    }
  }
  return chosen;
}
