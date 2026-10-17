import { locale } from '../../shared/i18n/locale.svelte';
import type { HostApi, LoudnessJob, LoudnessJobRequest, SessionSnapshot, StartLoudnessArgs } from '@sungeki/api-client';
export const loudnessActive = (job: LoudnessJob) => ['queued', 'running', 'cancelling'].includes(job.phase);
const phaseKeys = { queued: 'loudnessQueued', running: 'loudnessRunning', cancelling: 'loudnessCancelling', cancelled: 'loudnessCancelled', succeeded: 'loudnessSucceeded', failed: 'loudnessFailed', interrupted: 'loudnessInterrupted' } as const;
export const loudnessPhase = (phase: LoudnessJob['phase']) => locale.text(phaseKeys[phase]);

export class LoudnessJobsController {
  jobs = $state<LoudnessJob[]>([]);
  selected = $state<LoudnessJob | null>(null);
  pending = $state.raw<StartLoudnessArgs | null>(null);
  busy = $state(false);
  error = $state('');
  more = $state(false);
  private after: string | null = null;
  private alive = true;
  constructor(private api: Pick<HostApi, 'session'>, readonly sessionId: string) {}
  dispose() { this.alive = false; }
  private async run(action: () => Promise<void>) {
    if (this.busy || !this.alive) return;
    this.busy = true; this.error = '';
    try { await action(); }
    catch (e) { if (this.alive) this.error = e && typeof e === 'object' && 'message' in e ? String(e.message) : String(e); }
    finally { if (this.alive) this.busy = false; }
  }
  private async job(request: LoudnessJobRequest, id: string) {
    const response = await this.api.session({ type: 'loudness_job', request });
    if (response.type !== 'loudness_job' || response.job.session_id !== this.sessionId || response.job.job_id !== id) {
      throw new Error(locale.text('loudnessMismatch'));
    }
    return response.job;
  }
  private accept(job: LoudnessJob) {
    if (!this.alive) return;
    this.selected = job;
    this.jobs = [job, ...this.jobs.filter(old => old.job_id !== job.job_id)];
  }
  async start(snapshot: SessionSnapshot, assetId: string) {
    if (this.pending || snapshot.session_id !== this.sessionId) return;
    const asset = snapshot.project.assets.find(a => a.id === assetId);
    if (asset?.probe.type !== 'audio' || asset.locator.type !== 'absolute') { this.error = locale.text('loudnessSourceRequired'); return; }
    await this.run(async () => {
      this.pending = { session_id: this.sessionId, job_id: crypto.randomUUID(), expected_revision: snapshot.revision, asset_id: assetId };
      await this.submit();
    });
  }
  private async submit() {
    const args = this.pending;
    if (!args) return;
    try {
      const job = await this.job({ type: 'start', args }, args.job_id);
      if (!this.alive) return;
      if (job.asset_id !== args.asset_id || job.source_revision !== args.expected_revision) throw new Error(locale.text('loudnessTargetMismatch'));
      this.pending = null; this.accept(job);
    } catch (e) {
      if (!this.alive) return;
      const code = e && typeof e === 'object' && 'code' in e ? e.code : '';
      if (['VALIDATION_FAILED', 'CONFLICT', 'NOT_FOUND', 'PROTOCOL_MISMATCH', 'CAPABILITY_UNAVAILABLE'].includes(String(code))) this.pending = null;
      throw e;
    }
  }
  async retry() { if (this.pending) await this.run(() => this.submit()); }
  async refresh(id = this.selected?.job_id) {
    if (!id || this.pending) return;
    await this.run(async () => this.accept(await this.job({ type: 'get', job_id: id, wait_ms: 0 }, id)));
  }
  async cancel() {
    const id = this.selected?.job_id;
    if (!id || this.pending) return;
    await this.run(async () => this.accept(await this.job({ type: 'cancel', job_id: id }, id)));
  }
  async list(next = false) {
    if (this.pending) return;
    await this.run(async () => {
      const response = await this.api.session({ type: 'loudness_job', request: {
        type: 'list', session_id: this.sessionId, after: next ? this.after : null, limit: 12,
      } });
      if (!this.alive) return;
      if (response.type !== 'loudness_jobs' || response.job_ids.length > 12) throw new Error(locale.text('loudnessInvalidList'));
      const jobs: LoudnessJob[] = [];
      for (const id of response.job_ids) {
        if (!this.alive) return;
        jobs.push(await this.job({ type: 'get', job_id: id, wait_ms: 0 }, id));
      }
      if (!this.alive) return;
      this.jobs = next ? [...this.jobs, ...jobs.filter(job => !this.jobs.some(old => old.job_id === job.job_id))] : jobs;
      this.after = response.job_ids.at(-1) ?? null; this.more = response.job_ids.length === 12;
      if (this.selected) this.selected = jobs.find(job => job.job_id === this.selected?.job_id) ?? this.selected;
    });
  }
}
