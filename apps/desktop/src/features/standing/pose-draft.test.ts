import { expect, test } from 'vitest';
import { render } from 'svelte/server';
import type { Project } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/psd-project.json';
import { selectPoseOption } from './pose-draft';
import PsdPoseFields from './PsdPoseFields.svelte';

test('changing a parent removes descendant overrides but retains unrelated and similarly prefixed groups', () => {
  const pose = { source_sha256: 'a'.repeat(64), selections: [
    { group: '/1', option: '/1/0' }, { group: '/1/0/2', option: '/1/0/2/0' },
    { group: '/10', option: '/10/0' }, { group: '/2', option: '/2/0' },
  ] };
  const before = structuredClone(pose);
  const changed = selectPoseOption(pose, '/1', '/1/1');
  expect(changed.selections).toEqual([pose.selections[2], pose.selections[3], { group: '/1', option: '/1/1' }]);
  expect(selectPoseOption(changed, '/1', '').selections).toEqual(pose.selections.slice(2));
  expect(pose).toEqual(before);
});

test('saved pose options have readable labels and an explicit original-state choice', () => {
  const project = fixture as Project;
  const asset = project.assets[0]; const clip = project.clips[0];
  if (asset.probe.type !== 'psd' || clip.content.type !== 'psd') throw new Error('PSD required');
  const { body } = render(PsdPoseFields, { props: { probe: asset.probe.info, pose: clip.content.pose } });
  expect(body).toContain('表情・ポーズ');
  expect(body).toContain('元PSDの表示状態');
  expect(body).toContain('value="/0/1" selected');
  expect(body).toContain('>blue</option>');
});
