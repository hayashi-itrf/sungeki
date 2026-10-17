import { expect, test } from 'vitest';
import { PlaybackMetrics } from './playback-metrics';
test('pause, seek and visibility boundaries do not become playback stall samples', () => {
  const m = new PlaybackMetrics(); m.start();
  m.draw(0, true); m.draw(20, true); m.draw(5000, false); m.draw(9000, true); m.draw(9030, true);
  m.frame(0, 1, 1.01, true); m.frame(33, 1.033, 1.043, true);
  m.resetSegment(); m.frame(9000, 8, 8.01, true);
  expect(m.draws).toEqual([20,30]); expect(m.frameIntervals).toEqual([33]);
  expect(m.report().excludedObservations).toBe(1);
  expect(m.report().mediaClockMinusFramePtsMs.p95).toBeCloseTo(10);
  m.stop(); m.draw(10000, true); expect(m.draws).toHaveLength(2);
});
test('empty, reset, invalid and bounded samples remain explicit', () => {
  const m = new PlaybackMetrics(); expect(m.report().drawIntervalsMs.p95).toBeNull();
  m.start(); m.frame(NaN, 0, 0, true); expect(m.frameClockOffsets).toEqual([]);
  for(let i=0; i<3000; i++) m.draw(i*20,true);
  expect(m.draws).toHaveLength(2000); expect(m.report().drawIntervalsMs.p95).toBe(20);
  m.start(); expect(m.report().drawIntervalsMs.count).toBe(0);
});

test('rolling samples disclose totals and absolute offsets retain negative drift', () => {
  const m = new PlaybackMetrics(); m.start();
  for (let i = 0; i < 2101; i++) {
    m.draw(i * 20, true);
    m.frame(i * 33, i / 30 + .1, i / 30, true);
  }
  const report = m.report();
  expect(report.drawIntervalsMs.count).toBe(2000);
  expect(report.videoCallbackIntervalsMs.count).toBe(2000);
  expect(report.totalObservations).toEqual({ drawIntervals: 2100, videoCallbackIntervals: 2100,
    mediaClockFrameOffsets: 2101 });
  expect(report.mediaClockMinusFramePtsMs.p95).toBeCloseTo(-100);
  expect(report.absoluteMediaClockFrameOffsetMs.p95).toBeCloseTo(100);
  m.stop(); m.frame(999999, 0, 0, true);
  expect(m.report().totalObservations).toEqual(report.totalObservations);
  m.start();
  expect(m.report().totalObservations).toEqual({ drawIntervals: 0, videoCallbackIntervals: 0,
    mediaClockFrameOffsets: 0 });
  expect(m.report().absoluteMediaClockFrameOffsetMs.p95).toBeNull();
});

test('frame evidence retains an early worst offset after rolling samples expire', () => {
  const m = new PlaybackMetrics(); m.start(1000);
  m.frame(1100, 0, 1.6, true, { observedAtMs: 1102, expectedDisplayTimeMs: 1110,
    presentationTimeMs: 1098, presentedFrames: 1 });
  for (let i = 1; i <= 2100; i++) m.frame(1100 + i * 33, i / 30, i / 30 + .01, true);
  const trace = m.report().frameTrace;
  expect(trace.total).toBe(2101);
  expect(trace.first).toHaveLength(8); expect(trace.recent).toHaveLength(8);
  expect(trace.first[0].elapsedMs).toBe(100);
  expect(trace.worstAbsoluteOffset?.offsetMs).toBe(1600);
  expect(trace.worstAbsoluteOffset?.metadata?.observedAtMs).toBe(1102);
  expect(trace.recent.at(-1)?.frameTimeSeconds).toBe(70);
  expect(m.report().absoluteMediaClockFrameOffsetMs.max).toBeCloseTo(10);
});

test('frame evidence never treats pause or excluded samples as continuous advancement', () => {
  const m = new PlaybackMetrics(); m.start(100);
  m.frame(100, 0, 0, true); m.frame(133, .033, .034, true);
  let trace = m.report().frameTrace;
  expect(trace.recent[1].callbackGapMs).toBe(33);
  expect(trace.recent[1].frameAdvanceMs).toBe(33);
  expect(trace.recent[1].mediaAdvanceMs).toBe(34);
  m.frame(1000, 1, 9, false); m.frame(2000, 2, 2.01, true);
  trace = m.report().frameTrace;
  expect(trace.total).toBe(3); expect(trace.recent[2].callbackGapMs).toBeNull();
  m.resetSegment(); m.frame(3000, 3, 3.01, true);
  expect(m.report().frameTrace.recent[3].mediaAdvanceMs).toBeNull();
  m.stop(); m.frame(4000, 4, 8, true); expect(m.report().frameTrace.total).toBe(4);
  m.start(5000); expect(m.report().frameTrace).toMatchObject({ total: 0, first: [], recent: [], worstAbsoluteOffset: null });
});
