import { locale } from '../../shared/i18n/locale.svelte';
import type { Command, Project } from '@sungeki/api-client';
export type SpeechDraft = {
  characterId: string; subtitle: string; spoken: string; explicit: boolean;
  trackId: string; sceneId: string; start: number; duration: number;
};
// Only suggests an input value. The shared editor still owns validation and the actual edit.
export function nextSpeechStart(project: Project, trackId: string): number | null {
  const speech = project.clips.filter(c => c.track_id === trackId && c.content.type === 'speech');
  if (!speech.length) return null;
  const end = speech.reduce((last, clip) => Math.max(last, clip.start_us + clip.duration_us), 0);
  return (end + 350_000) / 1_000_000;
}
export function speechCommands(project: Project, draft: SpeechDraft): { commands: Command[]; clipId: string } {
  const start = Math.round(draft.start * 1e6), duration = Math.round(draft.duration * 1e6);
  if (![start, duration].every(Number.isSafeInteger) || start < 0 || duration < 1 || start + duration > 604800e6) {
    throw new Error(locale.text('speechInvalidTime'));
  }
  if (!project.characters.some(c => c.id === draft.characterId)) throw new Error(locale.text('speechInvalidCharacter'));
  if (draft.trackId && !project.tracks.some(t => t.id === draft.trackId && t.kind === 'audio')) throw new Error(locale.text('speechInvalidTrack'));
  if (draft.sceneId && !project.scenes.some(s => s.id === draft.sceneId)) throw new Error(locale.text('speechInvalidScene'));
  const clipId = crypto.randomUUID(), trackId = draft.trackId || crypto.randomUUID();
  const commands: Command[] = [];
  if (!draft.trackId) commands.push({ type: 'add_track', track: { id: trackId, name: 'ナレーション', kind: 'audio', visible: true, muted: false } });
  const { width, height } = project.canvas;
  commands.push({ type: 'add_clip', clip: {
    id: clipId, track_id: trackId, scene_id: draft.sceneId || null, start_us: start, duration_us: duration,
    transform: { x: width * .05, y: height * .8, width: width * .9, height: height * .15, opacity: 1 },
    content: { type: 'speech', gain_db: null, audio: null, character_id: draft.characterId, subtitle_text: draft.subtitle, spoken_text: draft.explicit ? draft.spoken : null },
  } });
  return { commands, clipId };
}
