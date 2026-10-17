import { expect, test } from 'vitest';
import type { Project } from '@sungeki/api-client';
import { audioLabel } from './label';
test('same named audio is distinguished by speech and start time, with shared assets explicit', () => {
  const project = { assets: [{ id: 'a', name: '合成音声' }, { id: 'b', name: '合成音声' }],
    clips: [{ start_us: 1250000, content: { type: 'speech', subtitle_text: 'こんにちは\nずんだもん', audio: { asset_id: 'a' } } },
      { start_us: 5000000, content: { type: 'speech', subtitle_text: '次の発話', audio: { asset_id: 'b' } } }] } as Project;
  expect(audioLabel(project, 'a')).toBe('1.25秒：こんにちは ずんだもん');
  expect(audioLabel(project, 'b')).toBe('5.00秒：次の発話');
  project.clips.push(project.clips[0]!);
  expect(audioLabel(project, 'a')).toContain('2発話で共有');
  expect(audioLabel(project, 'missing')).toBe('現在の作品にない音声');
});
