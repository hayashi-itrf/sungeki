import { afterEach, expect, test } from 'vitest';
import type { Command, Project } from '@sungeki/api-client';
import { locale } from '../../shared/i18n/locale.svelte';
import { describeCommand } from './command-description';

afterEach(() => { locale.current = 'ja'; });
test('media review keeps numeric settings and project names while changing language', () => {
  const project = { scenes: [{ id: 'scene', name: '導入', start_us: 0, duration_us: 5000000 }] } as Project;
  const commands: Command[] = [
    { type: 'set_video', clip_id: 'clip', asset_id: 'asset', playback: { source_start_us: 1250000, source_duration_us: 2500000,
      looped: true, audio_stream: null, gain_db: 0 } },
    { type: 'refresh_scene_credits', scene_id: 'scene', clip_id: 'clip' },
    { type: 'layout_speech', clip_ids: ['a', 'b'], start_us: 3000000, gap_us: 250000 },
  ];
  const original = structuredClone({ project, commands });
  locale.current = 'en';
  expect(describeCommand(commands[0], project)).toBe('Use 2.5 s of source video starting at 1.25 s (repeat; no source audio).');
  expect(describeCommand(commands[1], project)).toContain('scene “導入”');
  expect(describeCommand(commands[2], project)).toContain('2 speech clips using their generated audio lengths from 3 s with gaps of 0.25 s');
  locale.current = 'ja';
  expect(describeCommand(commands[0], project)).toContain('動画の使用区間を1.25秒から2.5秒間');
  expect({ project, commands }).toEqual(original);
});
