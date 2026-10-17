import { expect, test } from 'vitest';
import type { Project } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/sprite-project.json';
import { addPose, removePose, renamePose } from './sprite-draft';
const project = fixture as Project;
const original = project.clips[0].content;
if (original.type !== 'sprite') throw new Error('sprite fixture required');
const fresh = () => structuredClone(original);

test('renaming the selected default keeps both references attached and isolates the draft', () => {
  const value = { ...fresh(), pose: '通常' };
  const before = JSON.stringify(value);
  const renamed = renamePose(value, 0, '落ち着いた顔');
  expect(renamed.pose).toBe('落ち着いた顔');
  expect(renamed.sprites.default_pose).toBe('落ち着いた顔');
  expect(renamed.sprites.poses[0].closed_asset_id).toBe(value.sprites.poses[0].closed_asset_id);
  expect(JSON.stringify(value)).toBe(before);
  renamed.sprites.poses[1].name = 'another draft';
  expect(JSON.stringify(value)).toBe(before);
});

test('duplication finds a free name and retains both mouth images without changing selection', () => {
  const value = addPose(addPose(fresh()));
  expect(value.sprites.poses.slice(2).map(p => p.name)).toEqual(['新しいポーズ 1', '新しいポーズ 2']);
  expect(value.sprites.poses[2].open_asset_id).toBe(original.sprites.poses[0].open_asset_id);
  expect(value.pose).toBeNull();
  expect(value.sprites.default_pose).toBe('通常');
});

test('deletion refuses live references; changing them explicitly permits removal', () => {
  const value = { ...fresh(), pose: '笑顔' };
  expect(() => removePose(value, 0)).toThrow();
  expect(() => removePose(value, 1)).toThrow();
  value.pose = '通常';
  const removed = removePose(value, 1);
  expect(removed.sprites.poses.map(p => p.name)).toEqual(['通常']);
  expect(() => removePose(removed, 0)).toThrow();
});

test('duplicate and oversized names cannot leave ambiguous references', () => {
  for (const name of ['笑顔', '', '  ', 'bad\nname', 'あ'.repeat(86)]) expect(() => renamePose(fresh(), 0, name)).toThrow();
  expect(renamePose(fresh(), 0, 'あ'.repeat(85)).sprites.default_pose).toHaveLength(85);
  const value = fresh();
  value.sprites.poses = Array.from({ length: 256 }, (_, i) => ({ ...value.sprites.poses[0], name: String(i) }));
  expect(() => addPose(value)).toThrow('256');
});
