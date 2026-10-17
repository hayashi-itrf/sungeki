type Source = Pick<HTMLVideoElement, 'requestVideoFrameCallback' | 'cancelVideoFrameCallback'>;
// One decoded-frame subscription. Generation guards callbacks already queued at stop/seek.
export class VideoFrameLoop {
  private source: Source | undefined;
  private pending: number | undefined;
  private generation = 0;
  constructor(private paint: () => void, private eligible: () => boolean) {}
  start(source: Source) {
    this.stop();
    if (typeof source.requestVideoFrameCallback !== 'function' || !this.eligible()) return;
    this.source = source;
    const generation = this.generation;
    const request = () => {
      this.pending = source.requestVideoFrameCallback(() => {
        if (generation !== this.generation) return;
        this.pending = undefined;
        if (!this.eligible()) return;
        this.paint();
        // paint may fail and stop the loop, or the media may have been paused.
        if (generation === this.generation && this.eligible()) request();
      });
    };
    request();
  }
  stop() {
    this.generation++;
    if (this.pending !== undefined) this.source?.cancelVideoFrameCallback(this.pending);
    this.pending = undefined; this.source = undefined;
  }
}
