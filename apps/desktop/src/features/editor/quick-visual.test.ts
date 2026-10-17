import { expect, test } from 'vitest';
import type { Clip } from '@sungeki/api-client';
import { visualCommands, visualDraft } from './quick-visual';
const clip: Clip = { id: 'clip', track_id: 'track', scene_id: null, start_us: 0, duration_us: 5000000,
  transform: { x: 10, y: 20, width: 300, height: 100, opacity: 1,
    fade: { in_us: 100000, out_us: 200000 },
    motion: { end_x: 40, end_y: 50, end_scale: 1.2, easing: 'linear' } },
  content: { type: 'text', text: '本文を保持', font_size: 32, color: '#000000' } };
test('geometry sends only a shared transform command and preserves motion/fade/content/timing', () => {
  const original = structuredClone(clip);
  expect(visualCommands(clip, { ...visualDraft(clip), x: 42 })).toEqual([
    { type: 'set_transform', clip_id: clip.id, transform: { ...clip.transform, x: 42 } },
  ]);
  expect(clip).toEqual(original);
});
test('style adjustment never commits text, metadata, timing or audio drafts', () => {
  expect(visualCommands(clip, { ...visualDraft(clip), color: '#123456', fontSize: 44 })).toEqual([
    { type: 'set_text_style', clip_id: clip.id, font_size: 44, color: '#123456' },
  ]);
});
test('unchanged and reverted input creates no Undo step', () => {
  expect(visualCommands(clip, visualDraft(clip))).toEqual([]);
});
test('incomplete or invalid input cannot be sent as an edit', () => {
  for (const patch of [{ x: NaN }, { width: 0 }, { height: -1 }, { opacity: 1.1 }, { fontSize: 0 }, { color: '#' }])
    expect(visualCommands(clip, { ...visualDraft(clip), ...patch })).toBeNull();
});
test('shape and blur adjustments use their existing shared commands', () => {
  const shape: Clip = { ...clip, content: { type: 'shape', shape: 'rectangle', fill: '#ffffff' } };
  expect(visualCommands(shape, { ...visualDraft(shape), shape: 'ellipse' })).toEqual([
    { type: 'set_shape', clip_id: clip.id, shape: 'ellipse', fill: '#ffffff' },
  ]);
  const blur: Clip = { ...clip, content: { type: 'blur', sigma: 10 } };
  expect(visualCommands(blur, { ...visualDraft(blur), sigma: 20 })).toEqual([
    { type: 'set_blur', clip_id: clip.id, sigma: 20 },
  ]);
  expect(visualCommands(blur, { ...visualDraft(blur), sigma: 129 })).toBeNull();
});
