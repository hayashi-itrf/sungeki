import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { percentile, uiPerformance as meter } from './ui-performance.svelte';
let now: number, sequence: number;
let callbacks: Map<number, (time: number) => void>;
let visibility: (() => void) | undefined;
beforeEach(() => {
  now = 0; sequence = 0; callbacks = new Map(); visibility = undefined;
  vi.stubGlobal('performance', { now: () => now });
  vi.stubGlobal('document', { hidden: false,
    addEventListener: (_: string, callback: () => void) => { visibility = callback; },
    removeEventListener: () => { visibility = undefined; } });
  vi.stubGlobal('requestAnimationFrame', (callback: (time: number) => void) => { callbacks.set(++sequence, callback); return sequence; });
  vi.stubGlobal('cancelAnimationFrame', (id: number) => callbacks.delete(id));
  meter.clear(); meter.setEnabled(true);
});
afterEach(() => { meter.setEnabled(false); vi.unstubAllGlobals(); });
function frame(time: number) {
  now = time; const pending = [...callbacks.values()]; callbacks.clear(); pending.forEach(callback => callback(time));
}
test('measurement requires opt-in; successful edit observes two frame boundaries after response', async () => {
  meter.setEnabled(false); expect(meter.beginEdit()).toBeNull();
  meter.setEnabled(true); const finish = meter.beginEdit()!;
  now = 80; finish(true); await Promise.resolve(); await Promise.resolve();
  frame(96); expect(meter.edits).toHaveLength(0);
  frame(112); expect(meter.edits).toEqual([{ responseMs: 80, updateMs: 112 }]);
  const failed = meter.beginEdit()!; failed(false); await Promise.resolve(); frame(150); frame(170);
  expect(meter.edits).toHaveLength(1);
});
test('reset invalidates late frame observations and clears callbacks', async () => {
  meter.beginEdit()!(true); await Promise.resolve(); await Promise.resolve();
  meter.clear(); frame(100); frame(120); expect(meter.edits).toEqual([]); expect(callbacks.size).toBe(0);
});
test('drag records frame intervals only for sustained visible gestures', () => {
  const short = meter.beginDrag()!; frame(16); frame(32); short(); expect(meter.dragCount).toBe(0);
  const finish = meter.beginDrag()!; frame(100); frame(116); frame(132); now = 400; finish(); finish();
  expect(meter.dragCount).toBe(1); expect(meter.dragIntervals).toEqual([16, 16]);
  const hidden = meter.beginDrag()!;
  frame(420); Object.assign(document, { hidden: true }); visibility!();
  Object.assign(document, { hidden: false }); frame(1000); hidden(); expect(meter.dragCount).toBe(1);
});
test('p95 uses nearest rank without changing observations', () => {
  const values = [30, 10, 20]; expect(percentile(values, .95)).toBe(30); expect(values).toEqual([30, 10, 20]);
  expect(percentile([], .95)).toBeNull();
});
test('exclusion reasons distinguish hidden, short, failed, and missing frame observations', () => {
  meter.setEnabled(false); expect(meter.beginDrag()).toBeNull();
  expect(Object.values(meter.exclusions).reduce((a, b) => a + b)).toBe(0);
  meter.setEnabled(true);
  Object.assign(document, { hidden: true }); expect(meter.beginEdit()).toBeNull();
  Object.assign(document, { hidden: false });
  const short = meter.beginDrag()!; now = 100; short(); short();
  const noFrames = meter.beginDrag()!; now = 500; noFrames();
  const failed = meter.beginEdit()!; failed(false); failed(false);
  expect(meter.exclusions).toEqual({ hidden: 1, interrupted: 0, failed: 1, short: 1, noFrames: 1 });
  expect(meter.edits).toHaveLength(0); expect(meter.dragCount).toBe(0);
});
test('visibility changes account for pending samples once and clearing rejects stale callbacks', async () => {
  const edit = meter.beginEdit()!, drag = meter.beginDrag()!;
  edit(true); await Promise.resolve(); await Promise.resolve(); frame(20);
  Object.assign(document, { hidden: true }); visibility!(); visibility!();
  Object.assign(document, { hidden: false }); now = 400; drag(); frame(416);
  expect(meter.exclusions.hidden).toBe(2);
  expect(meter.edits).toHaveLength(0); expect(meter.dragCount).toBe(0);
  const stopped = meter.beginEdit()!; meter.setEnabled(false); stopped(true);
  expect(meter.exclusions.interrupted).toBe(1);
  meter.setEnabled(true); const stale = meter.beginDrag()!; meter.clear(); now = 1000; stale();
  expect(Object.values(meter.exclusions).reduce((a, b) => a + b)).toBe(0);
});

test('hidden startup is exposed before any gesture and visibility resumes eligible measurements', () => {
  meter.setEnabled(false);
  Object.assign(document, { hidden: true });
  meter.setEnabled(true);
  expect(meter.hidden).toBe(true);
  expect(meter.beginDrag()).toBeNull();
  Object.assign(document, { hidden: false }); visibility!();
  expect(meter.hidden).toBe(false);
  const finish = meter.beginDrag()!;
  frame(16); frame(32); frame(48); now = 400; finish();
  expect(meter.dragCount).toBe(1);
  expect(meter.dragIntervals).toEqual([16, 16]);
  expect(meter.exclusions.hidden).toBe(1);
});
