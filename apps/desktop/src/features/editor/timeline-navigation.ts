import type { EditorController } from './controller.svelte';
import type { Project } from '@sungeki/api-client';

// Match Project::duration: display padding is not part of the saved movie.
export function projectDuration(project: Pick<Project, 'clips' | 'scenes'>) {
  let end = 0;
  for (const item of project.clips) end = Math.max(end, item.start_us + item.duration_us);
  for (const item of project.scenes) end = Math.max(end, item.start_us + item.duration_us);
  return end / 1_000_000;
}

type FrameRate = { numerator: number; denominator: number };
type NavigationKey = Pick<KeyboardEvent, 'key' | 'shiftKey' | 'ctrlKey' | 'metaKey' | 'altKey' | 'isComposing' | 'defaultPrevented' | 'preventDefault'>;

// Calculate from an integer frame index instead of accumulating rounded microseconds.
export function seekTime(time: number, duration: number, fps: FrameRate, key: string, shift: boolean) {
  const rate = fps.numerator / fps.denominator;
  const last = Math.max(0, Math.ceil(duration * rate - 1e-8) - 1);
  const position = time * rate;
  const step = shift ? 10 : 1;
  const frame = key === 'Home' ? 0 : key === 'End' ? last
    : key === 'ArrowRight' ? Math.floor(position + 1e-8) + step
    : Math.ceil(position - 1e-8) - step;
  return Math.max(0, Math.min(last, frame)) / rate;
}

export function followTime(time: number, start: number, span: number, duration: number) {
  if (time >= start && time < start + span) return start;
  return Math.max(0, Math.min(time - span / 4, duration - span));
}

export function handleTimelineKey(event: NavigationKey, editor: EditorController, duration: number,
  context: { editingText: boolean; modalOpen: boolean }) {
  if (event.defaultPrevented || event.isComposing || event.ctrlKey || event.metaKey || event.altKey
    || context.editingText || context.modalOpen
    || !['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
  event.preventDefault();
  if (!editor.snapshot || editor.busy || editor.pending || editor.drafting) return;
  editor.time = seekTime(editor.time, duration, editor.snapshot.project.canvas.fps, event.key, event.shiftKey);
  editor.seekGeneration++;
}
