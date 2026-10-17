import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'svelte/server';
import type { Project } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/scene-evaluation.json';
import { locale } from '../../shared/i18n/locale.svelte';
import { EditorController } from './controller.svelte';
import Inspector from './Inspector.svelte';
import QuickVisual from './QuickVisual.svelte';
afterEach(() => { locale.current = 'ja'; });

test('text inspector translates controls without translating project text or changing draft values', () => {
  const project = structuredClone(fixture.project) as Project;
  const clip = project.clips.find(c => c.content.type === 'text')!;
  if (clip.content.type !== 'text') throw Error('text fixture required');
  clip.scene_id = null;
  clip.content.text = '日本語の作品名を保持';
  clip.content.layout = { alignment: 'right', wrap: true };
  const original = structuredClone(project);
  const editor = new EditorController({ session: vi.fn() });
  editor.snapshot = { session_id: 's', revision: 0, dirty: false, document_path: null,
    event_cursor: { host_instance_id: 'h', event_seq: 0 }, undo_operation_id: null, redo_operation_id: null, project };
  editor.selected = clip.id;
  locale.current = 'en';
  const english = render(Inspector, { props: { editor } }).body + render(QuickVisual, { props: { editor } }).body;
  for (const label of ['Selected clip', 'Text alignment', 'Wrap text to fit the width', 'Font size', 'Apply changes', 'Duplicate at the same position', 'Choose Text color', 'Text color (hex)']) {
    expect(english).toContain(label);
  }
  expect(english).toContain('value="right" selected');
  expect(english).toContain('日本語の作品名を保持');
  expect(english).not.toContain('文字の配置');
  locale.current = 'ja';
  const japanese = render(Inspector, { props: { editor } }).body + render(QuickVisual, { props: { editor } }).body;
  expect(japanese).toContain('文字の配置');
  expect(japanese).toContain('文字色のカラーコード');
  expect(project).toEqual(original);
});
