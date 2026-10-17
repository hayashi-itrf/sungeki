import { expect, test } from 'vitest';
import type { Clip, ClipContent, Project } from '@sungeki/api-client';
import { assetUses } from './targets';

function clip(id: string, start_us: number, content: ClipContent): Clip {
  return { id, start_us, duration_us: 1_000_000, track_id: 'visual', scene_id: null,
    transform: { x: 0, y: 0, width: 100, height: 100, opacity: 1 }, content };
}
function uses(clips: Clip[], asset: string) {
  return assetUses({ clips } as Project, asset).map(clip => clip.id);
}
test('video sources are discoverable by their registered asset', () => {
  const video = clip('video', 0, { type: 'video', asset_id: 'movie', playback: {} as never });
  expect(uses([video], 'movie')).toEqual(['video']);
  expect(uses([video], 'other')).toEqual([]);
});
test('all sprite poses and both mouth states count, without duplicate navigation entries', () => {
  const sprite = clip('sprite', 0, { type: 'sprite', pose: 'normal', character_id: null,
    sprites: { default_pose: 'normal', poses: [
      { name: 'normal', closed_asset_id: 'closed', open_asset_id: 'open' },
      { name: 'alternate', closed_asset_id: 'alternate', open_asset_id: 'open' },
    ] } });
  for (const id of ['closed', 'open', 'alternate']) expect(uses([sprite], id)).toEqual(['sprite']);
  expect(uses([sprite], 'unrelated')).toEqual([]);
});
test('VRM model and motion uses retain chronological ordering across clips', () => {
  const content: ClipContent = { type: 'vrm', standing: { model_asset_id: 'model', motion_asset_id: 'motion',
    source_start_us: 0, character_id: null, expressions: {} } };
  const clips = [clip('later', 2_000_000, content), clip('first', 0, content)];
  for (const id of ['model', 'motion']) expect(uses(clips, id)).toEqual(['first', 'later']);
  expect(uses(clips, 'other')).toEqual([]);
});
