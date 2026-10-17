import type { Clip, Project } from '@sungeki/api-client';
type Range = { start: number; end: number };
export type SpritePreview = { error: string; at: (timeUs: number) => string | null };

// Read-only projection of a host-validated snapshot. Integer boundary fixtures
// are shared with Rust ScenePlan; this never edits Project or invents analysis.
export function spritePreview(project: Project, clip: Clip): SpritePreview {
  const fail = (error: string): SpritePreview => ({ error, at: () => null });
  if (clip.content.type !== 'sprite') return fail('スプライトではありません');
  const { sprites, pose: selected, character_id: character } = clip.content;
  const pose = sprites.poses.find(p => p.name === (selected ?? sprites.default_pose));
  if (!pose) return fail('指定したポーズがありません');
  const end = clip.start_us + clip.duration_us;
  const ranges: Range[] = [];
  const muted = new Set(project.tracks.filter(t => t.muted).map(t => t.id));
  if (character && pose.open_asset_id) {
    for (const speech of project.clips) {
      const content = speech.content;
      const speechEnd = speech.start_us + speech.duration_us;
      if (content.type !== 'speech' || content.character_id !== character || muted.has(speech.track_id)
        || speechEnd <= clip.start_us || speech.start_us >= end) continue;
      const timing = content.audio?.lip_sync;
      if (!timing || !Number.isSafeInteger(timing.sample_rate) || timing.sample_rate <= 0) {
        return fail('口パク解析が必要です');
      }
      const rate = BigInt(timing.sample_rate);
      const absolute = (frame: number) => speech.start_us + Number((BigInt(frame) * 1_000_000n + rate - 1n) / rate);
      for (const span of timing.open) {
        const start = Math.max(absolute(span.start_frame), clip.start_us);
        const stop = Math.min(absolute(span.end_frame), speechEnd, end);
        if (start < stop) {
          ranges.push({ start, end: stop });
          if (ranges.length > 50_000) return fail('立ち絵の開口区間が50,000を超えています');
        }
      }
    }
  }
  ranges.sort((a,b) => a.start - b.start || a.end - b.end);
  const merged: Range[] = [];
  for (const range of ranges) {
    const last = merged.at(-1);
    if (last && range.start <= last.end) last.end = Math.max(last.end, range.end);
    else merged.push({ ...range });
  }
  const visible = project.tracks.find(t => t.id === clip.track_id)?.visible;
  return { error: '', at(timeUs) {
    if (!visible || !Number.isSafeInteger(timeUs) || timeUs < clip.start_us || timeUs >= end) return null;
    let left = 0, right = merged.length;
    while (left < right) {
      const mid = (left + right) >>> 1;
      if (merged[mid].end <= timeUs) left = mid + 1;
      else right = mid;
    }
    return merged[left]?.start <= timeUs ? pose.open_asset_id! : pose.closed_asset_id;
  } };
}
