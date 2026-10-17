import { expect, test } from 'vitest';
import type { Clip } from '@sungeki/api-client';
import { placementCommand } from './placement-command';
const clip: Clip = {
  id: 'text', track_id: 'track', scene_id: null, start_us: 0, duration_us: 1_000_000,
  content: { type: 'text', text: 'タイトル', font_size: 32, color: '#000000' },
  transform: { x: 10, y: 20, width: 300, height: 100, opacity: 1 },
};
const draft = { ...clip.transform, flipX: false, fadeIn: 0, fadeOut: 0 };

test('unchanged placement emits no edit and does not normalize optional fields', () => {
  expect(placementCommand(clip, draft)).toBeNull();
  const explicit: Clip = { ...clip, transform: { ...clip.transform, flip_x: false, fade: { in_us: 0, out_us: 0 } } };
  expect(placementCommand(explicit, draft)).toBeNull();
  expect(clip.transform).not.toHaveProperty('flip_x');
});

test('a real geometry edit preserves motion, fade, and omitted flip', () => {
  const source: Clip = { ...clip, transform: { ...clip.transform,
    motion: { end_x: 20, end_y: 40, end_scale: 1.2, easing: 'linear' },
    fade: { in_us: 100000, out_us: 200000 } } };
  const original = structuredClone(source);
  expect(placementCommand(source, { ...draft, x: 50, fadeIn: .1, fadeOut: .2 })).toEqual({
    type: 'set_transform', clip_id: clip.id, transform: { ...source.transform, x: 50 },
  });
  expect(source).toEqual(original);
});

test('explicitly changing flip or clearing fade is retained', () => {
  const source: Clip = { ...clip, transform: { ...clip.transform, flip_x: true, fade: { in_us: 100000, out_us: 0 } } };
  expect(placementCommand(source, draft)).toEqual({ type: 'set_transform', clip_id: clip.id,
    transform: { ...clip.transform, flip_x: false } });
  expect(source.transform.fade).toEqual({ in_us: 100000, out_us: 0 });
});
