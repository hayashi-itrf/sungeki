import { expect, test, vi } from 'vitest';
import type { SessionSnapshot } from '@sungeki/api-client';
import { EditorController } from './controller.svelte';
import { followTime, handleTimelineKey, seekTime, projectDuration } from './timeline-navigation';
import fixture from '../../../../../tests/fixtures/scene-evaluation.json';
import type { Project } from '@sungeki/api-client';
const fps = { numerator: 30000, denominator: 1001 };
test('short movies end on their last frame rather than the five-second display padding', () => {
  const project = structuredClone(fixture.project) as Project;
  project.clips = [{ ...project.clips[0], start_us: 0, duration_us: 1_000_000 }];
  project.scenes = [];
  const before = JSON.stringify(project);
  expect(projectDuration(project)).toBe(1);
  expect(seekTime(0, projectDuration(project), { numerator: 30, denominator: 1 }, 'End', false)).toBe(29 / 30);
  project.scenes = [{ id: 'tail', name: 'tail', start_us: 1_000_000, duration_us: 2_500_000 }];
  expect(projectDuration(project)).toBe(3.5);
  expect(seekTime(0, projectDuration(project), fps, 'End', false)).toBe(104 * 1001 / 30000);
  project.scenes = [];
  expect(JSON.stringify(project)).toBe(before);
  expect(projectDuration({ clips: [], scenes: [] })).toBe(0);
});

test('scene navigation follows a zoomed viewport and preserves document, selection and drafts', () => {
  const session = vi.fn(); const editor = new EditorController({ session });
  editor.snapshot = { revision: 8, session_id: 'session', dirty: false, document_path: null,
    event_cursor: { host_instance_id: 'host', event_seq: 0 }, undo_operation_id: null, redo_operation_id: null,
    project: { ...fixture.project as Project, scenes: [
    { id: 'later', name: 'later', start_us: 40_000_000, duration_us: 10_000_000 },
  ] } };
  editor.selected = 'keep-selection';
  const before = JSON.stringify(editor.snapshot);
  expect(editor.revealScene('later')).toBe(true);
  expect(editor.time).toBe(40);
  expect(editor.seekGeneration).toBe(1);
  expect(followTime(editor.time, 0, 10, 50)).toBe(37.5);
  expect(editor.selected).toBe('keep-selection');
  expect(editor.revealScene('missing')).toBe(false);
  editor.drafting = true; expect(editor.revealScene('later')).toBe(false);
  expect(editor.drafting).toBe(true);
  editor.drafting = false; editor.busy = true; expect(editor.revealScene('later')).toBe(false);
  editor.busy = false; editor.pending = { type: 'get', session_id: 'session' };
  expect(editor.revealScene('later')).toBe(false);
  expect(editor.seekGeneration).toBe(1);
  expect(JSON.stringify(editor.snapshot)).toBe(before);
  expect(session).not.toHaveBeenCalled();
});
test('frame navigation avoids cumulative drift and remains within the last visible frame', () => {
  let time = 0;
  for (let i = 0; i < 10000; i++) time = seekTime(time, 1000, fps, 'ArrowRight', false);
  expect(time).toBeCloseTo(10000 * 1001 / 30000, 10);
  for (let i = 0; i < 10000; i++) time = seekTime(time, 1000, fps, 'ArrowLeft', false);
  expect(time).toBe(0);
  expect(seekTime(0, 5, { numerator: 30, denominator: 1 }, 'End', false)).toBe(149 / 30);
  expect(seekTime(5, 5, fps, 'ArrowRight', false)).toBe(149 * 1001 / 30000);
  expect(seekTime(0, 0, fps, 'End', false)).toBe(0);
  expect(seekTime(100, 1000, fps, 'Home', false)).toBe(0);
});
test('arbitrary scrub positions step in the requested direction; shift advances ten frames', () => {
  expect(seekTime(.05, 5, { numerator: 30, denominator: 1 }, 'ArrowLeft', false)).toBe(1 / 30);
  expect(seekTime(.05, 5, { numerator: 30, denominator: 1 }, 'ArrowRight', false)).toBe(2 / 30);
  expect(seekTime(1, 5, { numerator: 30, denominator: 1 }, 'ArrowRight', true)).toBe(40 / 30);
  expect(seekTime(0, 5, fps, 'ArrowLeft', true)).toBe(0);
});
test('zoom follows out-of-range positions without moving a visible position or exceeding duration', () => {
  expect(followTime(29, 20, 10, 60)).toBe(20);
  expect(followTime(30, 20, 10, 60)).toBe(27.5);
  expect(followTime(5, 20, 10, 60)).toBe(2.5);
  expect(followTime(59.9, 0, 10, 60)).toBe(50);
  expect(followTime(0, 20, 10, 60)).toBe(0);
});
test('navigation is local view state and preserves selection, Project and host history', () => {
  const session = vi.fn(); const editor = new EditorController({ session });
  editor.snapshot = { revision: 8, project: { canvas: { fps } } } as SessionSnapshot;
  editor.selected = 'clip';
  const before = JSON.stringify(editor.snapshot);
  const event = { key: 'ArrowRight', shiftKey: false, ctrlKey: false, metaKey: false, altKey: false,
    isComposing: false, defaultPrevented: false, preventDefault: vi.fn() };
  const context = { editingText: false, modalOpen: false };
  handleTimelineKey(event, editor, 60, context);
  expect(editor.time).toBe(1001 / 30000);
  expect(editor.selected).toBe('clip'); expect(editor.seekGeneration).toBe(1);
  for (const guard of [{ editingText: true }, { modalOpen: true }]) {
    handleTimelineKey(event, editor, 60, { ...context, ...guard });
  }
  for (const guard of [{ isComposing: true }, { ctrlKey: true }, { metaKey: true }, { altKey: true }, { defaultPrevented: true }]) {
    handleTimelineKey({ ...event, ...guard }, editor, 60, context);
  }
  editor.drafting = true; handleTimelineKey(event, editor, 60, context);
  editor.drafting = false; editor.busy = true; handleTimelineKey(event, editor, 60, context);
  editor.busy = false; editor.pending = { type: 'get', session_id: 'session' };
  handleTimelineKey(event, editor, 60, context);
  expect(editor.seekGeneration).toBe(1);
  expect(JSON.stringify(editor.snapshot)).toBe(before); expect(session).not.toHaveBeenCalled();
});
