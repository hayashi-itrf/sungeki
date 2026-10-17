import { motionTransform } from './visual-motion';
import type { Clip, Project } from '@sungeki/api-client';

// The host validates the immutable Project. This local evaluator is checked
// against the same boundary/order fixture as sungeki-scene; no IPC per frame.
export function previewClips(project: Project, timeUs: number): Clip[] {
  if (!Number.isSafeInteger(timeUs) || timeUs < 0) return [];
  const byTrack = new Map<string, Clip[]>();
  for (const clip of project.clips) {
    if (clip.content.type !== 'audio' && clip.start_us <= timeUs && timeUs < clip.start_us + clip.duration_us) {
      const clips = byTrack.get(clip.track_id) ?? [];
      const transform = motionTransform(clip.transform, timeUs - clip.start_us, clip.duration_us);
      const fade = transform.fade;
      if (fade) {
        const elapsed = timeUs - clip.start_us;
        const factor = Math.max(0, Math.min(1, fade.in_us ? elapsed / fade.in_us : 1,
          fade.out_us ? (clip.duration_us - elapsed) / fade.out_us : 1));
        clips.push({ ...clip, transform: { ...transform, opacity: transform.opacity * factor } });
      } else clips.push(transform === clip.transform ? clip : { ...clip, transform });
      byTrack.set(clip.track_id, clips);
    }
  }
  return project.tracks.filter(track => track.visible).flatMap(track => byTrack.get(track.id) ?? []);
}
