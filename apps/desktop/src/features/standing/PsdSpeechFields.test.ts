import { expect, test } from 'vitest';
import { render } from 'svelte/server';
import type { Project } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/psd-speech-project.json';
import PsdSpeechFields from './PsdSpeechFields.svelte';
import { psdOptions } from './psd-options';

const project = fixture as Project;
const asset = project.assets[0];
const content = project.clips[0].content;
if (asset.probe.type !== 'psd' || content.type !== 'psd') throw new Error('PSD fixture required');
const probe = asset.probe.info;
const speech = content.speech;

test('saved character and mouth choices appear with distinct accessible controls', () => {
  const { body } = render(PsdSpeechFields, { props: { probe, characters: project.characters, speech } });
  for (const text of ['発話するキャラクター', '口のグループ', '閉じた口', '開いた口', '表示時刻に合わせて口形']) expect(body).toContain(text);
  expect(body).toContain('value="/0/1" selected');
  expect(body).toContain('value="/0/0" selected');
  expect(body).toContain('value="/0/1" disabled');
  expect(body).toContain('required');
});

test('missing characters explain why enabling mouth animation is unavailable', () => {
  const { body } = render(PsdSpeechFields, { props: { probe, characters: [], speech: null } });
  expect(body).toContain('先にキャラクターを登録してください');
  expect(body).toMatch(/type="checkbox"[^>]*disabled/);
  expect(body).not.toContain('発話するキャラクター<select');
});

test('group options exclude non-options and descendants while retaining readable hierarchy', () => {
  const nested = structuredClone(probe);
  nested.layers.push({ path: '/0/2', name: 'nested', kind: 'group', visible: true },
    { path: '/0/2/0', name: '*child one', kind: 'pixel', visible: true },
    { path: '/0/2/1', name: '*child two', kind: 'pixel', visible: false });
  const groups = psdOptions(nested);
  expect(groups[0].options.map(option => option.path)).toEqual(['/0/0', '/0/1']);
  expect(groups[1].label).toBe('choice / nested');
  expect(groups[1].options.map(option => option.label)).toEqual(['child one', 'child two']);
});
