import { locale } from '../../shared/i18n/locale.svelte';
import type { BuildVideoJob, HostApi, RenderJob } from '@sungeki/api-client';

export class BuildPlayback {
  job = $state<RenderJob | null>(null);
  busy = $state(false);
  error = $state('');
  private alive = true;
  constructor(private api: Pick<HostApi, 'session'>) {}
  dispose() { this.alive = false; }
  async load(build: Pick<BuildVideoJob, 'session_id' | 'output'>) {
    if (!this.alive || this.busy || !build.output) return;
    const { render_job_id: id, render: expected } = build.output;
    this.busy = true; this.error = ''; this.job = null;
    try {
      const response = await this.api.session({ type: 'render_job', request: { type: 'get', job_id: id, wait_ms: 0 } });
      if (!this.alive) return;
      if (response.type !== 'render_job' || response.job.job_id !== id
        || response.job.session_id !== build.session_id || response.job.purpose !== 'final'
        || response.job.phase !== 'succeeded' || !response.job.output) {
        throw new Error(locale.text('buildPlaybackMissing'));
      }
      const actual = response.job.output;
      if (Object.keys(expected).some(key => actual[key as keyof typeof actual] !== expected[key as keyof typeof expected])) {
        throw new Error(locale.text('buildPlaybackMismatch'));
      }
      this.job = response.job;
    } catch (e) {
      if (this.alive) this.error = e && typeof e === 'object' && 'message' in e ? String(e.message) : String(e);
    } finally { if (this.alive) this.busy = false; }
  }
}
