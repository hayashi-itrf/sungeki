import { locale } from '../../shared/i18n/locale.svelte';
import type { HostApi, ReviewFrameOptions, ReviewJob, SessionRequest, SessionSnapshot } from '@sungeki/api-client';
import { reviewDiagnostic as message } from './review-diagnostic';
type Start = Extract<SessionRequest, { type: 'start_review' }>;
export const active = (job: ReviewJob) => ['queued', 'running', 'cancelling'].includes(job.phase);
export class ReviewController {
  jobs = $state<ReviewJob[]>([]);
  unavailable = $state<{ id: string; message: string }[]>([]);
  selected = $state<ReviewJob | null>(null);
  busy = $state(false);
  error = $state('');
  pending = $state.raw<Start | null>(null);
  after = $state<string | null>(null);
  more = $state(false);
  private generation = 0;
  private disposed = false;
  constructor(private api: Pick<HostApi, 'session'>, readonly sessionId: string) {}
  dispose() { this.disposed = true; this.generation++; }
  private async run(action: () => Promise<void>) {
    if (this.disposed || this.busy) return;
    this.busy = true; this.error = '';
    try { await action(); } catch (e) { if (!this.disposed) this.error = message(e); }
    finally { if (!this.disposed) this.busy = false; }
  }
  private async job(request: SessionRequest): Promise<ReviewJob> {
    const result = await this.api.session(request);
    if (result.type !== 'review_job' || result.job.session_id !== this.sessionId || !('job_id' in request) || result.job.job_id !== request.job_id) throw new Error(locale.text('reportResponseError'));
    return result.job;
  }
  private accept(job: ReviewJob) {
    this.unavailable = this.unavailable.filter(entry => entry.id !== job.job_id);
    this.selected = job;
    const index = this.jobs.findIndex(j => j.job_id === job.job_id);
    if (index >= 0) this.jobs[index] = job;
    else this.jobs.unshift(job);
  }
  async start(snapshot: SessionSnapshot, sampling?: ReviewFrameOptions) {
    if (this.disposed || snapshot.session_id !== this.sessionId || this.busy || this.pending) return;
    this.pending = { sampling: sampling ? { ...sampling, requested_times_us: [...sampling.requested_times_us] } : undefined, type: 'start_review', session_id: this.sessionId, job_id: crypto.randomUUID(), expected_revision: snapshot.revision, wait_ms: 0 };
    await this.retry();
  }
  async retry() {
    const request = this.pending;
    if (!request) return;
    const generation = this.generation;
    await this.run(async () => {
      try {
        const job = await this.job(request);
        if (generation !== this.generation) return;
        if (job.source_revision !== request.expected_revision) throw new Error(locale.text('reportResponseError'));
        this.pending = null; this.accept(job);
      } catch (e) {
        if (generation !== this.generation) return;
        const code = e && typeof e === 'object' && 'code' in e ? e.code : undefined;
        if (code && code !== 'HOST_UNAVAILABLE' && code !== 'IO_ERROR') this.pending = null;
        throw e;
      }
    });
  }
  async refresh(id = this.selected?.job_id) {
    if (!id || this.pending) return;
    const generation = this.generation;
    await this.run(async () => {
      try {
        const job = await this.job({ type: 'get_review_job', job_id: id, wait_ms: 0 });
        if (generation === this.generation) this.accept(job);
      } catch (e) {
        if (generation !== this.generation) return;
        this.unavailable = [...this.unavailable.filter(entry => entry.id !== id), { id, message: message(e) }];
        throw e;
      }
    });
  }
  async cancel() {
    const id = this.selected?.job_id;
    if (!id || this.pending) return;
    const generation = this.generation;
    await this.run(async () => {
      const job = await this.job({ type: 'cancel_review_job', job_id: id });
      if (generation === this.generation) this.accept(job);
    });
  }
  async list(next = false) {
    if (this.pending || (next && !this.more)) return;
    const generation = this.generation;
    await this.run(async () => {
      const response = await this.api.session({ type: 'list_review_jobs', session_id: this.sessionId, after: next ? this.after : null, limit: 12 });
      if (generation !== this.generation) return;
      if (response.type !== 'review_jobs' || response.job_ids.length > 12 || new Set(response.job_ids).size !== response.job_ids.length) throw new Error(locale.text('reportListError'));
      // Small bounded batches; do not read every stored Project/report at once.
      const jobs: ReviewJob[] = [];
      const unavailable: { id: string; message: string }[] = [];
      for (let offset = 0; offset < response.job_ids.length; offset += 3) {
        const ids = response.job_ids.slice(offset, offset + 3);
        const results = await Promise.allSettled(ids.map(job_id => this.job({ type: 'get_review_job', job_id, wait_ms: 0 })));
        if (generation !== this.generation) return;
        results.forEach((result, index) => {
          if (result.status === 'fulfilled') jobs.push(result.value);
          else unavailable.push({ id: ids[index], message: message(result.reason) });
        });
      }
      if (generation !== this.generation) return;
      this.jobs = next ? [...this.jobs, ...jobs.filter(j => !this.jobs.some(old => old.job_id === j.job_id))] : jobs;
      // A first-page refresh may omit the selected older report. It cannot
      // establish that this report has recovered from its last read failure.
      const retained = this.unavailable.filter(entry => !response.job_ids.includes(entry.id)
        && (next || entry.id === this.selected?.job_id));
      this.unavailable = [...retained, ...unavailable];
      this.after = response.job_ids.at(-1) ?? null; this.more = response.job_ids.length === 12;
      if (this.selected) {
        const current = jobs.find(j => j.job_id === this.selected?.job_id);
        if (current) this.selected = current;
      }
    });
  }
}
