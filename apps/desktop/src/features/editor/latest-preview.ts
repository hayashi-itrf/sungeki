// One in-flight image per view. New times replace the pending request, without
// cancelling every decoded frame during playback or building an unbounded queue.
export class LatestPreview<T> {
  private pending: T | null = null;
  private identity: string | null = null;
  private requestKey: string | undefined;
  private running = false;
  private version = 0;
  private disposed = false;
  constructor(private load: (value: T, cancelled: () => boolean) => Promise<string | null>,
    private show: (url: string) => void, private fail: (error: unknown) => void) {}
  select(identity: string) {
    if (identity === this.identity) return false;
    this.identity = identity; this.reset(); return true;
  }
  reset() { this.version++; this.pending = null; this.requestKey = undefined; }
  request(value: T, key?: string) {
    if (key !== undefined && key === this.requestKey) return;
    this.requestKey = key; this.pending = value; void this.run();
  }
  private async run() {
    if (this.running || this.disposed) return;
    this.running = true;
    while (this.pending !== null && !this.disposed) {
      const value = this.pending, version = this.version; this.pending = null;
      const cancelled = () => this.disposed || version !== this.version;
      try { const url = await this.load(value, cancelled); if (!cancelled() && url !== null) this.show(url); }
      catch (error) { if (!cancelled()) this.fail(error); }
    }
    this.running = false;
  }
  dispose() { this.disposed = true; this.reset(); }
}
