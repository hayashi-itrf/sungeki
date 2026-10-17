import { expect, test } from 'vitest';
import { render } from 'svelte/server';
import type { Project } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/vrm-clip-project.json';
import VrmFields from './VrmFields.svelte';
const project = fixture as Project;
const content = project.clips[0].content;
if (content.type !== 'vrm') throw new Error('VRM fixture required');

test('VRM settings distinguish model, motion, source offset and static expression', () => {
  const { body } = render(VrmFields, { props: { value: content.standing, assets: project.assets, characters: [] } });
  expect(body).toContain(`value="${content.standing.model_asset_id}" selected`);
  expect(body).toContain(`value="${content.standing.motion_asset_id}" selected`);
  expect(body).toContain('モーションの開始位置（秒）');
  expect(body).toContain('value="0.5"');
  expect(body).toContain('連動しない（指定した表情を使う）');
  expect(body).toContain('正式な画像を確認');
});
test('model changes expose unsupported expressions without silently deleting the draft', () => {
  const value = { ...content.standing, expressions: { missing: .7 } };
  const before = JSON.stringify(value);
  const { body } = render(VrmFields, { props: { value, assets: project.assets, characters: [] } });
  expect(body).toContain('このモデルにない表情「missing」');
  expect(body).toContain('「missing」の指定を解除');
  expect(JSON.stringify(value)).toBe(before);
});
