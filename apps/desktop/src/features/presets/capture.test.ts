import { expect, test } from 'vitest';
import type { Project } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/sprite-project.json';
import { captureCharacter, captureVoice } from './capture';

test('capture retains every sprite pose but excludes unrelated assets and detaches the source', () => {
  const project = structuredClone(fixture) as Project;
  const before = JSON.stringify(project);
  const c = project.characters[0]!;
  const clip = project.clips.find(c => c.content.type === 'sprite')!;
  const preset = captureCharacter(project, c.id, clip.id, '人物');
  expect(preset.content.kind).toBe('character');
  if (preset.content.kind !== 'character' || preset.content.standing?.type !== 'sprite') throw new Error();
  const referenced = new Set(preset.content.standing.sprites.poses.flatMap(p => [p.closed_asset_id, ...(p.open_asset_id ? [p.open_asset_id] : [])]));
  expect(new Set(preset.content.assets.map(a => a.id))).toEqual(referenced);
  expect(preset.content.assets.length).toBeLessThan(project.assets.length);
  preset.content.character.name = 'changed';
  preset.content.standing.sprites.poses[0]!.name = 'changed';
  expect(JSON.stringify(project)).toBe(before);
});

test('capture rejects a missing selected standing or its missing dependencies', () => {
  const project = structuredClone(fixture) as Project;
  const c = project.characters[0]!;
  expect(() => captureCharacter(project, c.id, 'missing', '人物')).toThrow();
  project.assets = [];
  expect(() => captureCharacter(project, c.id, project.clips[0]!.id, '人物')).toThrow();
  const preset = captureCharacter(project, c.id, '', '字幕だけ');
  expect(preset.content.kind === 'character' && preset.content.standing).toBeNull();
});

test('voice capture is detached and includes attribution without standing dependencies', () => {
  const project = structuredClone(fixture) as Project;
  const c = project.characters[0]!;
  const preset = captureVoice(project, c.id, '声');
  expect(preset.content).toEqual({ kind: 'voice', voice: c.voice, credit: c.credit });
  if (preset.content.kind !== 'voice') throw new Error();
  preset.content.credit = 'changed';
  expect(c.credit).not.toBe('changed');
  expect(() => captureVoice(project, 'missing', '声')).toThrow();
});
