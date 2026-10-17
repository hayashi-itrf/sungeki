import { expect, test } from 'vitest';
import type { Project } from '@sungeki/api-client';
import { newScene, scenesFromForm } from './scenes';
const project = { assets: [{ id: 'background', probe: { type: 'image' } }] } as unknown as Project;
function scene(gapSeconds: number | undefined) {
  return { ...newScene('本文'), background: 'background', gapSeconds };
}
test('scene-specific pauses reach the common recipe without changing script or form', () => {
  const input = [scene(0), scene(0.375), scene(60)];
  const original = structuredClone(input);
  const result = scenesFromForm(project, input);
  expect(result.map(s => s.gap_us)).toEqual([0, 375000, 60000000]);
  expect(result.every(s => s.lines.length === 2)).toBe(true);
  expect(input).toEqual(original);
  expect(newScene('新規').gapSeconds).toBe(0.15);
});
test('blank and invalid pauses fail with scene location instead of silently using the default', () => {
  for (const gap of [undefined, NaN, Infinity, -0.01, 60.001]) {
    expect(() => scenesFromForm(project, [scene(0.15), scene(gap)])).toThrow('場面2：セリフ間の余白');
  }
  expect(scenesFromForm(project, [scene(0.1234564)])[0]!.gap_us).toBe(123456);
});

test('scene fade remains optional and validates seconds before common layout', () => {
  const input = scene(0.15);
  expect(scenesFromForm(project, [input])[0]!.fade).toBeUndefined();
  const edited = { ...input, fade: { inSeconds: 0.3, outSeconds: 0.45 } };
  expect(scenesFromForm(project, [edited])[0]!.fade).toEqual({ in_us: 300000, out_us: 450000 });
  for (const fade of [{ inSeconds: undefined, outSeconds: 0 }, { inSeconds: -1, outSeconds: 0 },
    { inSeconds: Infinity, outSeconds: 0 }, { inSeconds: 604800, outSeconds: 1 }]) {
    expect(() => scenesFromForm(project, [{ ...input, fade }])).toThrow('場面1：フェード');
  }
});
