import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'svelte/server';
import { locale } from '../../shared/i18n/locale.svelte';
import { EditorController } from './controller.svelte';
import SceneFade from './SceneFade.svelte';
import { describeCommand } from '../scenes/command-description';
import type { Project } from '@sungeki/api-client';

afterEach(() => { locale.current = 'ja'; });
test('scene fade instructions and controls follow the selected language', () => {
  const session = vi.fn();
  const editor = new EditorController({ session });
  locale.current = 'en';
  const english = render(SceneFade, { props: { editor } }).body;
  for (const label of ['Whole-scene fade', 'Close', 'Fade in (seconds)', 'Fade out (seconds)',
    'Review changes', 'Discard input and load the latest state', 'Voice and BGM levels stay unchanged.',
    'Set both durations to zero to remove the fade.']) expect(english).toContain(label);
  expect(english).not.toContain('シーン全体のフェード');
  locale.current = 'ja';
  const japanese = render(SceneFade, { props: { editor } }).body;
  for (const label of ['シーン全体のフェード', '表示までの長さ（秒）', '消えるまでの長さ（秒）',
    '変更内容を確認', '音声とBGMの音量は変わりません']) expect(japanese).toContain(label);
  expect(session).not.toHaveBeenCalled();
});

test('fade review describes setting and removal in the selected language', () => {
  const project = { scenes: [] } as unknown as Project;
  const command = { type: 'set_scene_fade' as const, scene_id: 'scene', fade: { in_us: 800000, out_us: 1250000 } };
  locale.current = 'en';
  expect(describeCommand(command, project)).toBe('Fade the whole scene in from the background color over 0.8 seconds and out to it over 1.25 seconds');
  expect(describeCommand({ ...command, fade: null }, project)).toBe('Remove the whole-scene fade');
  locale.current = 'ja';
  expect(describeCommand(command, project)).toBe('シーン全体を背景色から0.8秒で表示し、1.25秒で背景色へ戻す');
  expect(describeCommand({ ...command, fade: null }, project)).toBe('シーン全体のフェードを解除する');
});
