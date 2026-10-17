import { expect, test } from 'vitest';
import type { Project } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/video-clip-project.json';
import { diagramsFromForm, selectDiagram, videoDuration } from './diagrams';
function project(): Project {
  const p = structuredClone(fixture) as Project;
  p.assets.push({ ...p.assets[0], id: 'image', probe: { type: 'image' } } as unknown as Project['assets'][number]);
  return p;
}
test('video selection keeps line range, explicitly starts silent and looped, and emits independent recipe data', () => {
  const p = project(), before = structuredClone(p);
  const row = selectDiagram(p, { asset: '', first: 2, last: 3 }, p.assets[0].id);
  expect(row.videoPlayback).toEqual({ source_start_us: 0, source_duration_us: 1000000, looped: true, audio_stream: null, gain_db: 0 });
  const recipe = diagramsFromForm(p, [row], 3)[0];
  expect(recipe.first_line).toBe(1); expect(recipe.last_line).toBe(2);
  expect(recipe.video_playback).toEqual(row.videoPlayback);
  recipe.video_playback!.gain_db = -12;
  expect(row.videoPlayback!.gain_db).toBe(0);
  expect(p).toEqual(before);
  const image = selectDiagram(p, row, 'image');
  expect(image.videoPlayback).toBeUndefined();
  expect(diagramsFromForm(p, [image], 3)[0]).toEqual({ asset_id: 'image', first_line: 1, last_line: 2 });
});
test('invalid video range, audio selection and unknown duration fail before submission', () => {
  const p = project();
  for (const patch of [{ source_start_us: -1 }, { source_duration_us: 0 }, { source_start_us: 1000001 }, { source_duration_us: NaN }, { audio_stream: 999 }, { gain_db: Infinity }]) {
    const row = selectDiagram(p, { asset: '', first: 1, last: 1 }, p.assets[0].id);
    Object.assign(row.videoPlayback!, patch);
    expect(() => diagramsFromForm(p, [row], 1)).toThrow('動画再生設定');
  }
  const probe = p.assets[0].probe;
  if (probe.type !== 'video') throw new Error('video required');
  probe.info.metadata.video.clock.duration_ts = null;
  expect(() => selectDiagram(p, { asset: '', first: 1, last: 1 }, p.assets[0].id)).toThrow('長さが不明');
});
test('fractional source duration rounds up by the same rational rule as common placement', () => {
  const probe = project().assets[0].probe;
  if (probe.type !== 'video') throw new Error('video required');
  probe.info.metadata.video.clock.duration_ts = 1;
  probe.info.metadata.video.clock.time_base = { numerator: 1, denominator: 3 };
  expect(videoDuration(probe.info)).toBe(333334);
});
