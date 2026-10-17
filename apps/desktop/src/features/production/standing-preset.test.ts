import { expect, test } from 'vitest';
import type { ClipContent } from '@sungeki/api-client';
import { standingPreset } from './standing-preset';
import zundamon from '../../../../../presets/characters/zundamon-sakamoto-2.3-mouth.json';
import metan from '../../../../../presets/characters/metan-sakamoto-2.1-mouth.json';
test('preset ID follows the standing original regardless of screen side or character name', () => {
  for (const preset of [metan, zundamon]) {
    const content = { type: 'psd', asset_id: 'arbitrary', pose: { source_sha256: preset.source_sha256, selections: [] }, speech: null } as ClipContent;
    expect(standingPreset(content)?.preset_id).toBe(preset.preset_id);
  }
  expect(standingPreset({ type: 'psd', asset_id: 'unknown', pose: { source_sha256: 'unknown', selections: [] }, speech: null })).toBeUndefined();
  expect(standingPreset({ type: 'image', asset_id: 'image' })).toBeUndefined();
  expect(standingPreset(undefined)).toBeUndefined();
});

test('managed voices use art-only metadata while VOICEVOX retains its voice-bound preset', () => {
  const content = { type: 'psd', asset_id: 'art', pose: { source_sha256: zundamon.source_sha256, selections: [] }, speech: null } as ClipContent;
  for (const engine of ['sbv2', 'qwen', 'irodori'] as const) {
    expect(standingPreset(content, engine)?.preset_id).toBe('zundamon-sakamoto-2.3-art-mouth');
    expect(standingPreset(content, engine)).toMatchObject({ voice_preset_id: null });
  }
  expect(standingPreset(content, 'voicevox')?.preset_id).toBe(zundamon.preset_id);
});
