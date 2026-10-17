import { expect, test } from 'vitest';
import type { Project } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/vrm-clip-project.json';
import { importVrm, type VrmImportSource } from './import-vrm';
const project = fixture as Project;
function source(index: number): VrmImportSource {
  const a = project.assets[index];
  if (a.probe.type !== 'vrm') throw Error('VRM fixture required');
  return { path: `/tmp/${a.name}`, credit: a.credit, probe: a.probe.info };
}
const ids = { model: 'm', motion: 'a', track: 't', clip: 'c' };
test('VRM import is one transaction with separate credits, scene and original references', () => {
  const before = JSON.stringify(project);
  const result = importVrm(project, source(0), source(1), .1, .5, project.scenes[0].id, '', ids);
  expect(result).toHaveLength(4);
  expect(result[0]).toMatchObject({ type: 'register_asset', asset: { id: 'm', credit: source(0).credit } });
  expect(result[1]).toMatchObject({ type: 'register_asset', asset: { id: 'a', credit: source(1).credit } });
  expect(result[3]).toMatchObject({ type: 'add_clip', clip: { scene_id: project.scenes[0].id, start_us: 100000, duration_us: 500000,
    content: { type: 'vrm', standing: { model_asset_id: 'm', motion_asset_id: 'a', character_id: null } } } });
  if (result[0].type !== 'register_asset' || result[0].asset.probe.type !== 'vrm') throw Error('expected model');
  result[0].asset.probe.info.inspection.expressions.length = 0;
  expect(JSON.stringify(project)).toBe(before);
});
test('bad time, swapped kinds, absent character and missing credits cannot produce imports', () => {
  for (const [start, duration] of [[NaN, 1], [0, Infinity], [-1, 1], [0, 0], [604800, 1]])
    expect(() => importVrm(project, source(0), source(1), start, duration, '', '', ids)).toThrow();
  expect(() => importVrm(project, source(1), source(0), 0, 1, '', '', ids)).toThrow('種類');
  expect(() => importVrm(project, { ...source(0), credit: ' ' }, source(1), 0, 1, '', '', ids)).toThrow('クレジット');
  expect(() => importVrm(project, source(0), source(1), 0, 1, '', 'missing', ids)).toThrow('キャラクター');
});
