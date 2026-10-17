import { test } from 'node:test';
import assert from 'node:assert/strict';
import { parseRenderRequest } from '../../workers/vrm/src/render-request.ts';
function fixture() {
  const source = { path: '/fixture/model', sha256: 'a'.repeat(64), byte_length: 100 };
  return { version: 1, width: 640, height: 360, model: source, motion: { ...source, path: '/fixture/motion' },
    frames: [{ time_us: 33367, expressions: { aa: 1 } }, { time_us: 0, expressions: { aa: 0 } }] };
}
test('render request preserves arbitrary order and isolates caller expression objects', () => {
  const input = fixture(); const parsed = parseRenderRequest(input);
  assert.deepEqual(parsed.frames.map(f => f.time_us), [33367, 0]);
  input.frames[0].expressions.aa = 0;
  assert.equal(parsed.frames[0].expressions.aa, 1);
});
test('render request rejects invalid identities, fields, budgets and expression values', () => {
  for (const change of [
    (r: any) => { r.version = 2; }, (r: any) => { r.width = 2049; },
    (r: any) => { r.model.path = '../outside'; }, (r: any) => { r.motion.sha256 = 'wrong'; },
    (r: any) => { r.frames = []; }, (r: any) => { r.frames[0].time_us = 1.5; },
    (r: any) => { r.frames[0].expressions.aa = NaN; }, (r: any) => { r.frames[0].expressions.aa = 2; },
    (r: any) => { r.frames[0].output = '/unexpected'; }, (r: any) => { r.output = '/unexpected'; },
  ]) { const input = fixture(); change(input); assert.throws(() => parseRenderRequest(input)); }
});
