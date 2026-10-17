import { expect, test } from 'vitest';
import type { Clip } from '@sungeki/api-client';
import { previewLayers } from './preview-layers';
const clip = (id: string, blur = false): Clip => ({ id, track_id: 't', scene_id: null, start_us: 0, duration_us: 1,
  transform: { x: 0, y: 0, width: 100, height: 100, opacity: 1 },
  content: blur ? { type: 'blur', sigma: 18 } : { type: 'text', text: id, font_size: 20, color: '#000000' } });
test('effects wrap the preceding backdrop and leave later captions outside', () => {
  const back = clip('back'), blur = clip('blur', true), front = clip('front');
  const original = JSON.stringify([back, blur, front]);
  const nodes = previewLayers([back, blur, front]);
  expect(nodes).toEqual([{ kind: 'blur', clip: blur, children: [{ kind: 'background' }, { kind: 'clip', clip: back }] },
    { kind: 'clip', clip: front }]);
  expect(JSON.stringify([back, blur, front])).toBe(original);
});
test('consecutive effects preserve order without duplicating the backdrop', () => {
  const a = clip('a', true), b = clip('b', true);
  expect(previewLayers([a, b])).toEqual([{ kind: 'blur', clip: b,
    children: [{ kind: 'blur', clip: a, children: [{ kind: 'background' }] }] }]);
});
test('disabled and zero-opacity effects do not nest or trigger image remounts', () => {
  const a = clip('a', true), b = clip('b', true), c = clip('c');
  if (a.content.type === 'blur') a.content.sigma = 0;
  b.transform.opacity = 0;
  expect(previewLayers([a, c, b])).toEqual([{ kind: 'background' }, { kind: 'clip', clip: c }]);
});
