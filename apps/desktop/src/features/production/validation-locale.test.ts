import { diagramsFromForm, type DiagramInput } from './diagrams';
import { newReaction } from './reaction';
import { afterEach, expect, test } from 'vitest';
import type { Project } from '@sungeki/api-client';
import { locale } from '../../shared/i18n/locale.svelte';
import { parseDialogue } from './dialogue';
import { newScene, scenesFromForm } from './scenes';
const project = { assets: [{ id: 'background', probe: { type: 'image' } }] } as unknown as Project;
afterEach(() => { locale.current = 'ja'; });
test('English script errors identify physical lines and retain the accepted Japanese directives', () => {
  locale.current = 'en';
  expect(() => parseDialogue('左：Hello\r\n\r\n読み：hello')).toThrow('Line 3: add reading:text');
  expect(() => parseDialogue('左：Hello\nポーズ：smile\nポーズ：other')).toThrow('Line 3: add pose:name');
  expect(() => parseDialogue('Left: Hello')).toThrow('Line 1: enter left:dialogue or right:dialogue');
  const script = '左： Hello \r\n読み： はろー \r\nポーズ： 笑顔 \r\n右：Reply';
  const english = parseDialogue(script);
  locale.current = 'ja';
  expect(parseDialogue(script)).toEqual(english);
  expect(english[0]).toMatchObject({ subtitle_text: ' Hello ', spoken_text: ' はろー ', sprite_pose: ' 笑顔 ' });
  expect(() => parseDialogue('左：Hello\n\n読み：hello')).toThrow('3行目の読み');
});
test('scene validation translates locations and limits without altering draft or recipe', () => {
  const input = [{ ...newScene('場面の原文'), background: 'background', gapSeconds: 0.375,
    fade: { inSeconds: 0.25, outSeconds: 0.5 } }];
  const before = structuredClone(input);
  const japanese = scenesFromForm(project, input);
  locale.current = 'en';
  expect(scenesFromForm(project, input)).toEqual(japanese);
  expect(input).toEqual(before);
  expect(() => scenesFromForm(project, [])).toThrow('Use 1–200 scenes.');
  expect(() => scenesFromForm(project, [input[0]!, { ...input[0]!, name: '' }])).toThrow('Scene 2: Enter a scene name.');
  expect(() => scenesFromForm(project, [{ ...input[0]!, background: 'missing' }])).toThrow('Scene 1: Select a background image.');
  expect(() => scenesFromForm(project, [{ ...input[0]!, gapSeconds: NaN }])).toThrow('Scene 1: Set the gap');
  expect(() => scenesFromForm(project, [{ ...input[0]!, fade: { inSeconds: -1, outSeconds: 0 } }])).toThrow('Scene 1: Set non-negative fade times');
  expect(() => scenesFromForm(project, [{ ...input[0]!, script: '' }])).toThrow('Scene 1: Use 1–200 dialogue lines.');
  expect(() => scenesFromForm(project, [{ ...input[0]!, script: '左：Hello\n読み：' }])).toThrow('Scene 1: Line 2: add reading:text');
  const many = { ...input[0]!, script: Array(101).fill('左：Hello').join('\n') };
  expect(() => scenesFromForm(project, [many, many])).toThrow('no more than 200 dialogue lines across all scenes');
});
test('diagram errors locate invalid ranges and preserve successful effect settings across languages', () => {
  const row = { asset: 'background', first: 1, last: 2 };
  const input: DiagramInput[] = [{ ...row, motion: { endX: 123, endY: 456, scale: 75, easing: 'linear' as const },
    fade: { inSeconds: 0.25, outSeconds: 0.5 } }, { ...row, reaction: newReaction(false) }];
  const before = structuredClone(input);
  const japanese = diagramsFromForm(project, input, 2);
  locale.current = 'en';
  expect(diagramsFromForm(project, input, 2)).toEqual(japanese);
  expect(input).toEqual(before);
  expect(() => diagramsFromForm(project, [{ ...row, asset: 'missing' }], 2)).toThrow('Select an image or video for diagram 1.');
  expect(() => diagramsFromForm(project, [{ ...row, last: 3 }], 2)).toThrow('diagram 1 within 1–2.');
  expect(() => diagramsFromForm(project, [row, row], 2)).toThrow('overlap at dialogue line 1.');
  expect(() => diagramsFromForm(project, [{ ...input[0]!, motion: { ...input[0]!.motion!, scale: 0 } }], 2)).toThrow('Check the diagram destination');
  expect(() => diagramsFromForm(project, [{ ...row, reaction: { ...newReaction(false), scale: 101 } }], 2)).toThrow('Check the reaction position');
  expect(() => diagramsFromForm(project, [{ ...row, reaction: { ...newReaction(false), fadeIn: 604801 } }], 2)).toThrow('reaction fade duration is too long');
  expect(() => diagramsFromForm(project, [{ ...row, reaction: newReaction(false), fade: { inSeconds: 0, outSeconds: 0 } }], 2)).toThrow('regular fade and a reaction cannot be combined');
});
