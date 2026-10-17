import { expect, test } from 'vitest';
import type { Project } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/video-clip-project.json';
import { newDiagramMotion, diagramMotionFromForm } from './diagram-motion';
import { diagramsFromForm, selectDiagram } from './diagrams';
import { newReaction } from './reaction';
const motion = { endX: 480, endY: 180, scale: 80, easing: 'ease_in_out' as const };
function project(): Project {
  const p = structuredClone(fixture) as Project;
  p.assets.push({ ...p.assets[0]!, id: 'image', probe: { type: 'image' } } as unknown as Project['assets'][number]);
  return p;
}
test('image motion reaches common recipe with speech timing and isolated form state', () => {
  const p = project(); const row = { asset: 'image', first: 1, last: 2, motion: { ...motion } };
  const output = diagramsFromForm(p, [row], 2)[0]!;
  expect(output).toMatchObject({ first_line: 0, last_line: 1, motion: { end_x: 480, end_y: 180, end_scale: 0.8, easing: 'ease_in_out' } });
  output.motion!.end_x = 0;
  expect(row.motion.endX).toBe(480);
  expect(selectDiagram(p, row, 'image').motion).toEqual(motion);
  expect(selectDiagram(p, row, p.assets[0]!.id).motion).toBeUndefined();
  expect(() => diagramsFromForm(p, [{ ...row, reaction: newReaction(false) }], 2)).toThrow('通常画像');
  expect(() => diagramsFromForm(p, [{ ...row, asset: p.assets[0]!.id }], 2)).toThrow('通常画像');
});
test('unset, invalid coordinates and scales are rejected before submission', () => {
  expect(() => diagramMotionFromForm(newDiagramMotion())).toThrow('移動先');
  for (const patch of [{ endX: NaN }, { endY: 1000001 }, { endX: -1000001 }, { scale: 0 }, { scale: Infinity }]) {
    expect(() => diagramMotionFromForm({ ...motion, ...patch })).toThrow('移動先');
  }
});

test('diagram fade survives image/video selection, validates seconds and conflicts', () => {
  const p = project();
  const row = { asset: 'image', first: 1, last: 2, motion: { ...motion }, fade: { inSeconds: 0.3, outSeconds: 0.2 } };
  expect(diagramsFromForm(p, [row], 2)[0]!.fade).toEqual({ in_us: 300000, out_us: 200000 });
  const video = selectDiagram(p, row, p.assets[0]!.id);
  expect(diagramsFromForm(p, [video], 2)[0]!.fade).toEqual({ in_us: 300000, out_us: 200000 });
  video.fade!.inSeconds = 0;
  expect(row.fade.inSeconds).toBe(0.3);
  for (const inSeconds of [undefined, -1, NaN, Infinity, 604801]) {
    expect(() => diagramsFromForm(p, [{ ...row, fade: { inSeconds, outSeconds: 0 } }], 2)).toThrow('フェード時間');
  }
  expect(() => diagramsFromForm(p, [{ ...row, motion: undefined, reaction: newReaction(false) }], 2)).toThrow('併用');
});
