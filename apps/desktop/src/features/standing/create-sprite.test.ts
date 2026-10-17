import { expect, test } from 'vitest';
import type { Project } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/sprite-project.json';
import { spriteCommands } from './create-sprite';
const project = fixture as Project;
const content = project.clips[0].content;
if (content.type !== 'sprite') throw new Error('sprite fixture required');

test('new standing uses a separate visible front track and keeps scene/timing and source state', () => {
  const before = JSON.stringify(project);
  const commands = spriteCommands(project, content, .125, .75, project.scenes[0].id, { clip: 'c', track: 't' });
  expect(commands[0]).toMatchObject({ type: 'add_track', track: { id: 't', kind: 'visual', visible: true } });
  const command = commands[1];
  if (command.type !== 'add_clip' || command.clip.content.type !== 'sprite') throw new Error('expected sprite');
  expect(command.clip).toMatchObject({ track_id: 't', start_us: 125000, duration_us: 750000, scene_id: project.scenes[0].id });
  const t = command.clip.transform;
  expect(t.width / t.height).toBe(.5);
  expect(t.height).toBeLessThanOrEqual(project.canvas.height);
  command.clip.content.sprites.poses[0].closed_asset_id = 'changed draft';
  expect(JSON.stringify(project)).toBe(before);
});

test('invalid time and absent default image cannot produce a command', () => {
  for (const [start, duration] of [[NaN, 1], [0, Infinity], [-1, 1], [0, 0], [604800, 1], [0, .0000001]]) {
    expect(() => spriteCommands(project, content, start, duration, '', { clip: 'c', track: 't' })).toThrow();
  }
  expect(() => spriteCommands({ ...project, assets: [] }, content, 0, 1, '', { clip: 'c', track: 't' })).toThrow('閉口画像');
});
