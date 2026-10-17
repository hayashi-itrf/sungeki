import { locale } from '../../shared/i18n/locale.svelte';
import type { HostApi, RenderJob, RenderJobRequest, SessionSnapshot, StartRenderArgs, RenderPurpose } from '@sungeki/api-client';
const message = (e: unknown) => e && typeof e === 'object' && 'message' in e ? String(e.message) : String(e);
export const renderActive = (job: RenderJob) => ['queued', 'running', 'cancelling'].includes(job.phase);
export const renderPhase = {
  get queued() { return locale.text('renderQueued'); },
  get running() { return locale.text('renderRunning'); },
  get cancelling() { return locale.text('renderCancelling'); },
  get cancelled() { return locale.text('renderCancelled'); },
  get succeeded() { return locale.text('renderSucceeded'); },
  get failed() { return locale.text('renderFailed'); },
  get interrupted() { return locale.text('renderInterrupted'); },
};

export class RenderJobsController {
  jobs = $state<RenderJob[]>([]);
  unavailable = $state<{ id: string; message: string }[]>([]);
  selected = $state<RenderJob | null>(null);
  pending = $state.raw<StartRenderArgs | null>(null);
  busy = $state(false);
  error = $state('');
  errorActions = $state<string[]>([]);
  more = $state(false);
  private after: string | null = null;
  private alive = true;
  private polling = false;
  private generation = 0;
  constructor(private api: Pick<HostApi, 'session'>, readonly sessionId: string, private purpose: RenderPurpose = 'final') {}
  dispose() { this.alive = false; }
  private async run(action: () => Promise<void>) {
    if (this.busy || !this.alive) return;
    this.generation++;
    this.busy = true; this.error = ''; this.errorActions = [];
    try { await action(); }
    catch (e) {
      if (this.alive) {
        this.error = message(e);
        if (e && typeof e === 'object' && 'suggested_actions' in e && Array.isArray(e.suggested_actions))
          this.errorActions = e.suggested_actions.filter((action): action is string => typeof action === 'string');
      }
    }
    finally { if (this.alive) this.busy = false; }
  }
  private async job(request: RenderJobRequest, id: string) {
    const response = await this.api.session({ type: 'render_job', request });
    if (response.type !== 'render_job' || response.job.session_id !== this.sessionId || response.job.job_id !== id) {
      throw new Error(locale.text('renderResponseMismatch'));
    }
    return response.job;
  }
  private accept(job: RenderJob) {
    if (!this.alive) return;
    this.unavailable = this.unavailable.filter(entry => entry.id !== job.job_id);
    this.selected = job;
    const index = this.jobs.findIndex(old => old.job_id === job.job_id);
    if (index < 0) this.jobs = [job, ...this.jobs];
    else this.jobs[index] = job;
  }
  async start(snapshot: SessionSnapshot) {
    if (this.pending || snapshot.session_id !== this.sessionId) return;
    await this.run(async () => {
      this.pending = { session_id: this.sessionId, job_id: crypto.randomUUID(), expected_revision: snapshot.revision };
      await this.submit();
    });
  }
  private async submit() {
    const args = this.pending;
    if (!args) return;
    try {
      const job = await this.job({ type: this.purpose === 'editing_preview' ? 'preview' : 'start', args }, args.job_id);
      if (!this.alive) return;
      if (job.purpose !== this.purpose) throw new Error(locale.text('renderPurposeMismatch'));
      if (job.source_revision !== args.expected_revision) throw new Error(locale.text('renderResponseMismatch'));
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
    await this.run(async () => {
      try { this.accept(await this.job({ type: 'get', job_id: id, wait_ms: 0 }, id)); }
      catch (e) {
        if (!this.alive) return;
        this.unavailable = [...this.unavailable.filter(entry => entry.id !== id), { id, message: message(e) }];
        throw e;
      }
    });
  }
  // Automatic observation must not toggle foreground loading/disabled UI.
  // A user action supersedes an older poll, including cancellation or selection.
  async poll() {
    const id = this.selected?.job_id;
    if (!id || this.busy || this.pending || this.polling || !this.alive) return;
    const generation = this.generation;
    this.polling = true;
    try {
      const job = await this.job({ type: 'get', job_id: id, wait_ms: 0 }, id);
      if (this.alive && generation === this.generation && this.selected?.job_id === id) this.accept(job);
    } catch (e) {
      if (this.alive && generation === this.generation && this.selected?.job_id === id)
        this.unavailable = [...this.unavailable.filter(entry => entry.id !== id), { id, message: message(e) }];
    } finally { this.polling = false; }
  }
  async cancel() {
    const id = this.selected?.job_id;
    if (!id || this.pending) return;
    await this.run(async () => this.accept(await this.job({ type: 'cancel', job_id: id }, id)));
  }
  async list(next = false) {
    if (this.pending || (next && !this.more)) return;
    await this.run(async () => {
      const response = await this.api.session({ type: 'render_job', request: {
        type: 'list', session_id: this.sessionId, after: next ? this.after : null, limit: 12,
      } });
      if (!this.alive) return;
      if (response.type !== 'render_jobs' || response.job_ids.length > 12 || new Set(response.job_ids).size !== response.job_ids.length) throw new Error(locale.text('renderInvalidList'));
      const jobs: RenderJob[] = [];
      const unavailable: { id: string; message: string }[] = [];
      for (const id of response.job_ids) {
        if (!this.alive) return;
        try { jobs.push(await this.job({ type: 'get', job_id: id, wait_ms: 0 }, id)); }
        catch (e) { unavailable.push({ id, message: message(e) }); }
      }
      if (!this.alive) return;
      this.jobs = next ? [...this.jobs, ...jobs.filter(job => !this.jobs.some(old => old.job_id === job.job_id))] : jobs;
      const retained = this.unavailable.filter(entry => !response.job_ids.includes(entry.id)
        && (next || entry.id === this.selected?.job_id));
      this.unavailable = [...retained, ...unavailable];
      this.after = response.job_ids.at(-1) ?? null; this.more = response.job_ids.length === 12;
      if (this.selected) this.selected = jobs.find(job => job.job_id === this.selected?.job_id) ?? this.selected;
    });
  }
}
