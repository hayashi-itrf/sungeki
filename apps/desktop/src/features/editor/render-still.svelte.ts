import { locale } from '../../shared/i18n/locale.svelte';
import type { HostApi, RenderFrame } from '@sungeki/api-client';

// Keep only the newest requested paused position. In-flight decoding may finish,
// but it must never paint over a later seek, playback, or a closed player.
export class RenderStill {
  image = $state<RenderFrame | null>(null);
  busy = $state(false);
  error = $state('');
  private requested: number | null = null;
  private generation = 0;
  private alive = true;
  constructor(private api: Pick<HostApi, 'session'>, private jobId: string) {}
  clear() { this.generation++; this.requested = null; this.image = null; this.error = ''; }
  dispose() { this.alive = false; this.clear(); }
  load(seconds: number) {
    if (!this.alive || !Number.isFinite(seconds) || seconds < 0) return;
    const time = Math.round(seconds * 1_000_000);
    if (time === this.requested && (this.busy || this.image)) return;
    this.requested = time; this.image = null; this.error = '';
    if (!this.busy) void this.read();
  }
  private async read() {
    this.busy = true;
    try {
      while (this.alive && this.requested !== null) {
        const time = this.requested, generation = this.generation;
        try {
          const response = await this.api.session({ type: 'render_job', request: {
            type: 'frame', args: { job_id: this.jobId, time_us: time },
          } });
          if (this.alive && generation === this.generation && time === this.requested) {
            if (response.type !== 'render_frame' || response.image.job_id !== this.jobId
              || response.image.time_us !== time) throw new Error(locale.text('renderResponseMismatch'));
            this.image = response.image;
          }
        } catch (error) {
          if (this.alive && generation === this.generation && time === this.requested) {
            this.error = error && typeof error === 'object' && 'message' in error ? String(error.message) : String(error);
          }
        }
        if (generation === this.generation && time === this.requested) break;
      }
    } finally { this.busy = false; }
  }
}
