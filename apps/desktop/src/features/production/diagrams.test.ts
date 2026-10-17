import { expect, test } from 'vitest';
import type { Project } from '@sungeki/api-client';
import { diagramsFromForm } from './diagrams';
import { scriptRows } from './dialogue';
const project = { assets: [{ id: 'figure', probe: { type: 'image' } }, { id: 'sound', probe: { type: 'audio' } }] } as unknown as Project;
test('display numbering ignores blank lines and preserves dialogue content', () => {
  expect(scriptRows('左：本文。\r\n \r\n右：次。\r左：終わり。')).toEqual(['左：本文。', '右：次。', '左：終わり。']);
  expect(diagramsFromForm(project, [{ asset: 'figure', first: 1, last: 2 }, { asset: 'figure', first: 3, last: 3 }], 3)).toEqual([
    { asset_id: 'figure', first_line: 0, last_line: 1 }, { asset_id: 'figure', first_line: 2, last_line: 2 },
  ]);
});
test('invalid ranges and overlapping inclusive endpoints cannot silently change timing', () => {
  for (const [first, last] of [[0, 1], [2, 1], [1, 4], [1.5, 2], [undefined, 2], [1, NaN]]) {
    expect(() => diagramsFromForm(project, [{ asset: 'figure', first, last }], 3)).toThrow('セリフ範囲');
  }
  expect(() => diagramsFromForm(project, [{ asset: 'figure', first: 1, last: 2 }, { asset: 'figure', first: 2, last: 3 }], 3)).toThrow('セリフ2');
  expect(() => diagramsFromForm(project, [{ asset: 'sound', first: 1, last: 1 }], 3)).toThrow('画像');
  expect(() => diagramsFromForm(project, Array(201).fill({ asset: 'figure', first: 1, last: 1 }), 3)).toThrow('200');
});
