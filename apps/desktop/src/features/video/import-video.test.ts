import { expect, test } from 'vitest';
import type { Project, VideoProbe } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/video-clip-project.json';
import { displaySize, importVideo } from './import-video';
const project = fixture as Project;
function probe(): VideoProbe {
  const p = structuredClone(project.assets[0].probe);
  if (p.type !== 'video') throw new Error('video required');
  return p.info;
}
const ids = { asset: 'asset', track: 'track', clip: 'clip' };
test('imports inspected original at playhead with explicit silent audio and immutable metadata', () => {
  const p = probe(), before = structuredClone(p);
  const result = importVideo('/video.mov', p, 1234567, project.canvas, ids);
  const clip = result[2];
  if (clip.type !== 'add_clip' || clip.clip.content.type !== 'video') throw new Error('video required');
  expect(clip.clip.start_us).toBe(1234567);
  expect(clip.clip.duration_us).toBe(1000000);
  expect(clip.clip.content.playback.audio_stream).toBeNull();
  expect(clip.clip.scene_id).toBeNull();
  expect(p).toEqual(before);
});
test('uses exact rational duration and rejects unknown clock and timeline overflow', () => {
  const p = probe(); p.metadata.video.clock.duration_ts = 1;
  p.metadata.video.clock.time_base = { numerator: 1, denominator: 3 };
  const result = importVideo('/x.mov', p, 0, project.canvas, ids)[2];
  if (result.type !== 'add_clip') throw new Error('clip required');
  expect(result.clip.duration_us).toBe(333334);
  expect(() => importVideo('/x.mov', p, 604800000000 - 333333, project.canvas, ids)).toThrow();
  p.metadata.video.clock.start_pts = null;
  expect(() => importVideo('/x.mov', p, 0, project.canvas, ids)).toThrow();
});
test('placement respects rotation, pixel aspect and full matrix rather than lossy rotation', () => {
  const p = probe(); const s = p.metadata.video;
  s.width = 320; s.height = 180; s.sample_aspect_ratio = { numerator: 2, denominator: 1 };
  s.display_matrix = null; s.rotation_degrees = 90;
  expect(displaySize(s)).toEqual([180, 640]);
  s.display_matrix = '0: -65536 0 0\n1: 0 65536 0\n2: 0 0 1073741824';
  expect(displaySize(s)).toEqual([640, 180]);
  const result = importVideo('/x.mov', p, 0, project.canvas, ids)[2];
  if (result.type !== 'add_clip') throw new Error('clip required');
  expect(result.clip.transform).toEqual({ x: 0, y: 45, width: 320, height: 90, opacity: 1 });
  s.display_matrix = '0: 65536 0 1\n1: 0 65536 0\n2: 0 0 1073741824';
  expect(() => displaySize(s)).toThrow();
});
