import { afterEach, expect, test } from 'vitest';
import { locale } from '../../shared/i18n/locale.svelte';
afterEach(() => { locale.current = 'ja'; });
import { render } from 'svelte/server';
import type { Project } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/sprite-project.json';
import SpriteFields from './SpriteFields.svelte';

const project = fixture as Project;
const content = project.clips[0].content;
if (content.type !== 'sprite') throw new Error('sprite fixture required');

test('saved pose, speech binding, and image names have distinct controls', () => {
  const value = { ...content, pose: '笑顔' };
  const { body } = render(SpriteFields, { props: { value, assets: project.assets, characters: project.characters } });
  expect(body).toContain('value="笑顔" selected');
  expect(body).toContain(`value="${project.characters[0].id}" selected`);
  expect(body).toContain('閉口画像（通常）');
  expect(body).toContain('開口画像（通常）');
  expect(body).toContain('このポーズには開口画像がない');
  expect(body).toContain('mouth20');
  expect(body).not.toContain('synthetic PSD');
  expect(body).toContain('同じ寸法');
});

test('default selection and missing characters explain static display without claiming preview support', () => {
  const value = { ...content, character_id: null };
  const { body } = render(SpriteFields, { props: { value, assets: project.assets, characters: [] } });
  expect(body).toContain('既定のポーズ（通常）');
  expect(body).toContain('連動しない（閉じた口で表示）');
  expect(body).toContain('先にキャラクターを登録してください');
  expect(body).toContain('正式な画像を確認');
  expect(body).not.toContain('このポーズには開口画像がない');
  expect(content.character_id).not.toBeNull();
});


test('English labels preserve user pose names literally and leave saved content untouched', () => {
  locale.current = 'en';
  const value = structuredClone(content);
  value.sprites.poses[0].name = '通常 $&';
  value.sprites.default_pose = '通常 $&';
  value.pose = null;
  const before = structuredClone(value);
  const { body } = render(SpriteFields, { props: { value, assets: project.assets, characters: [] } });
  expect(body).toContain('Default pose (通常 $&amp;)');
  expect(body).toContain('Closed-mouth image (通常 $&amp;)');
  expect(body).toContain('Open-mouth image (通常 $&amp;)');
  expect(body).toContain('Register a character first');
  expect(body).not.toContain('{name}');
  expect(value).toEqual(before);
});
