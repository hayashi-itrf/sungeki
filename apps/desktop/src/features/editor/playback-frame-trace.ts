// Bounded raw observations for investigating timing differences, not an A/V verdict.
export type FrameTimingMetadata = {
  observedAtMs: number;
  expectedDisplayTimeMs: number;
  presentationTimeMs: number;
  presentedFrames: number;
};
export type FrameTimingSample = {
  elapsedMs: number;
  callbackNowMs: number;
  frameTimeSeconds: number;
  mediaTimeSeconds: number;
  offsetMs: number;
  callbackGapMs: number | null;
  frameAdvanceMs: number | null;
  mediaAdvanceMs: number | null;
  metadata?: FrameTimingMetadata;
};
export class PlaybackFrameTrace {
  private startedAtMs = 0;
  private first: FrameTimingSample[] = [];
  private recent: FrameTimingSample[] = [];
  private worst: FrameTimingSample | null = null;
  private previous: FrameTimingSample | null = null;
  private total = 0;
  start(now: number) {
    this.startedAtMs = now; this.first = []; this.recent = [];
    this.worst = null; this.previous = null; this.total = 0;
  }
  resetSegment() { this.previous = null; }
  frame(now: number, frameTime: number, mediaTime: number, metadata?: FrameTimingMetadata) {
    const previous = this.previous;
    const sample: FrameTimingSample = {
      elapsedMs: now - this.startedAtMs, callbackNowMs: now,
      frameTimeSeconds: frameTime, mediaTimeSeconds: mediaTime,
      offsetMs: (mediaTime - frameTime) * 1000,
      callbackGapMs: previous ? now - previous.callbackNowMs : null,
      frameAdvanceMs: previous ? (frameTime - previous.frameTimeSeconds) * 1000 : null,
      mediaAdvanceMs: previous ? (mediaTime - previous.mediaTimeSeconds) * 1000 : null,
      ...(metadata ? { metadata: { ...metadata } } : {}),
    };
    this.total++; this.previous = sample;
    if (this.first.length < 8) this.first.push(sample);
    this.recent.push(sample); if (this.recent.length > 8) this.recent.shift();
    if (!this.worst || Math.abs(sample.offsetMs) > Math.abs(this.worst.offsetMs)) this.worst = sample;
  }
  report() {
    return { startedAtMs: this.startedAtMs, total: this.total,
      first: this.first.slice(), recent: this.recent.slice(), worstAbsoluteOffset: this.worst,
      scope: 'First 8, most recent 8 and worst absolute offset; eligible callbacks only, not physical A/V latency' };
  }
}
