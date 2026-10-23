import { afterEach, expect, test } from 'vitest';
import { parse } from 'svelte/compiler';
import { locale } from './locale.svelte';
import { uiText } from './extended-ui';
import { extraText } from './extra-ui';
import { parseDialogue } from '../../features/production/dialogue';

afterEach(() => { locale.current = 'ja'; });

test('standing controls and diagnostics switch languages without reloading', () => {
  locale.current = 'en';
  expect(uiText('ポーズを適用')).toBe('Apply pose');
  expect(uiText('立ち絵の口パク')).toBe('Character lip sync');
  expect(extraText('キャラクター名を入力してください。')).toBe('Enter a character name.');
  locale.current = 'ja';
  expect(uiText('ポーズを適用')).toBe('ポーズを適用');
});

test('English script directives preserve user text and pose names in both languages', () => {
  for (const language of ['ja', 'en'] as const) {
    locale.current = language;
    const result = parseDialogue('left:日本語の本文\nreading:にほんご\npose:笑顔\nright:Hello.');
    expect(result[0]).toMatchObject({ role: 'left', subtitle_text: '日本語の本文', spoken_text: 'にほんご', sprite_pose: '笑顔' });
    expect(result[1]).toMatchObject({ role: 'right', subtitle_text: 'Hello.' });
    expect(() => parseDialogue('pose:smile')).toThrow();
    expect(() => parseDialogue('left:Hello\nreading:one\nreading:two')).toThrow();
  }
});

test('Svelte controls do not embed untranslated Japanese text or attributes', () => {
  const files = import.meta.glob('../../**/*.svelte', { query: '?raw', import: 'default', eager: true });
  const failures: string[] = [];
  function inspect(node: unknown, file: string) {
    if (!node || typeof node !== 'object') return;
    const value = node as Record<string, unknown>;
    if (value.type === 'Text' && typeof value.data === 'string' && /[ぁ-んァ-ヶ一-龠]/.test(value.data)) {
      // Language names are intentionally shown in their own language.
      if (!(file.endsWith('/AppearanceSettings.svelte') && value.data === '日本語')) failures.push(`${file}: ${value.data}`);
    }
    for (const [key, child] of Object.entries(value)) {
      if (['parent', 'loc'].includes(key)) continue;
      if (Array.isArray(child)) child.forEach(item => inspect(item, file));
      else if (child && typeof child === 'object') inspect(child, file);
    }
  }
  for (const [file, source] of Object.entries(files)) inspect(parse(source as string, { modern: true }).fragment, file);
  expect(failures).toEqual([]);
});
