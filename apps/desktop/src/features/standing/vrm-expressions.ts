import type { Project, VrmStanding } from '@sungeki/api-client';
/** Same sample-to-microsecond ceiling and half-open bounds as scene/mouth_timing. */
export function vrmExpressions(project: Project, standing: VrmStanding, timeUs: number) {
  if (!standing.character_id) return standing;
  const muted = new Set(project.tracks.filter(t => t.muted).map(t => t.id));
  let open = false;
  for (const clip of project.clips) {
    const content = clip.content;
    if (content.type !== 'speech' || content.character_id !== standing.character_id
      || muted.has(clip.track_id) || timeUs < clip.start_us || timeUs >= clip.start_us + clip.duration_us) continue;
    const timing = content.audio?.lip_sync;
    if (!timing) continue; // Unrendered speech has no audio; show closed mouth in the editing preview.
    const rate = BigInt(timing.sample_rate);
    const absolute = (frame: number) => clip.start_us + Number((BigInt(frame) * 1_000_000n + rate - 1n) / rate);
    open ||= timing.open.some(span => absolute(span.start_frame) <= timeUs && timeUs < absolute(span.end_frame));
  }
  return { ...standing, expressions: { ...standing.expressions, aa: open ? 1 : 0 } };
}
