import { PlaybackFrameTrace, type FrameTimingMetadata } from './playback-frame-trace';
// Opt-in local QA observations. Callback timing is not physical display/audio latency.
export class PlaybackMetrics {
  active = false;
  private trace = new PlaybackFrameTrace();
  draws: number[] = [];
  frameIntervals: number[] = [];
  frameClockOffsets: number[] = [];
  skipped = 0;
  private totalDrawIntervals = 0;
  private totalFrameIntervals = 0;
  private totalFrameOffsets = 0;
  private previousDraw: number | null = null;
  private previousFrame: number | null = null;
  start(startedAtMs = 0) {
    this.trace.start(startedAtMs);
    this.draws = []; this.frameIntervals = []; this.frameClockOffsets = [];
    this.skipped = 0; this.totalDrawIntervals = 0;
    this.totalFrameIntervals = 0; this.totalFrameOffsets = 0;
    this.resetSegment(); this.active = true;
  }
  stop() { this.active = false; this.resetSegment(); }
  resetSegment() { this.previousDraw = null; this.previousFrame = null; this.trace.resetSegment(); }
  private append(list: number[], value: number) { list.push(value); if (list.length > 2000) list.shift(); }
  draw(now: number, eligible: boolean) {
    if (!this.active) return;
    if (!eligible || !Number.isFinite(now)) { this.skipped++; this.resetSegment(); return; }
    if (this.previousDraw !== null && now > this.previousDraw) {
      this.totalDrawIntervals++; this.append(this.draws, now - this.previousDraw);
    }
    this.previousDraw = now;
  }
  frame(now: number, mediaTime: number, clockTime: number, eligible: boolean, metadata?: FrameTimingMetadata) {
    if (!this.active) return;
    if (!eligible || !Number.isFinite(now + mediaTime + clockTime)) { this.skipped++; this.resetSegment(); return; }
    if (this.previousFrame !== null && now > this.previousFrame) {
      this.totalFrameIntervals++; this.append(this.frameIntervals, now - this.previousFrame);
    }
    this.previousFrame = now;
    this.trace.frame(now, mediaTime, clockTime, metadata);
    this.totalFrameOffsets++;
    this.append(this.frameClockOffsets, (clockTime - mediaTime) * 1000);
  }
  report() {
    const summarize = (values: number[]) => {
      if (!values.length) return { count: 0, min: null, p95: null, max: null };
      const sorted = [...values].sort((a,b) => a-b);
      return { count: values.length, min: sorted[0], p95: sorted[Math.ceil(sorted.length * .95)-1], max: sorted.at(-1) };
    };
    return { frameTrace: this.trace.report(), drawIntervalsMs: summarize(this.draws), videoCallbackIntervalsMs: summarize(this.frameIntervals),
      mediaClockMinusFramePtsMs: summarize(this.frameClockOffsets),
      absoluteMediaClockFrameOffsetMs: summarize(this.frameClockOffsets.map(Math.abs)),
      totalObservations: { drawIntervals: this.totalDrawIntervals, videoCallbackIntervals: this.totalFrameIntervals,
        mediaClockFrameOffsets: this.totalFrameOffsets },
      aggregation: 'Most recent 2000 eligible observations per series; not whole-run statistics',
      excludedObservations: this.skipped,
      limit: 2000, scope: 'Canvas calls and video callbacks; not physical display or audio-device latency' };
  }
}
