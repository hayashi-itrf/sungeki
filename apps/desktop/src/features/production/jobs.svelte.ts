import { extraText } from '../../shared/i18n/extra-ui';
import { locale } from '../../shared/i18n/locale.svelte';
import type { BuildVideoJob, BuildVideoRequest, HostApi, StartBuildVideoArgs } from '@sungeki/api-client';
import type { StartJournal } from './start-journal';
// Two preparation steps precede speech; six layout/output/report steps follow it.
export const recoverySpeechStep = (job: BuildVideoJob) => job.phase === 'interrupted'
  && !!job.child_job_id && job.completed_steps >= 2 && job.completed_steps < job.total_steps - 6;
export const buildActive = (job: BuildVideoJob) => ['queued', 'running', 'cancelling'].includes(job.phase);
export const buildPhase = {
  get queued() { return locale.text('buildQueued'); },
  get running() { return locale.text('buildRunning'); },
  get cancelling() { return locale.text('buildCancelling'); },
  get cancelled() { return locale.text('buildCancelled'); },
  get succeeded() { return locale.text('buildSucceeded'); },
  get failed() { return locale.text('buildFailedPhase'); },
  get interrupted() { return locale.text('buildInterrupted'); },
};

export class BuildJobsController {
  jobs = $state<BuildVideoJob[]>([]);
  selected = $state<BuildVideoJob | null>(null);
  busy = $state(false);
  error = $state('');
  more = $state(false);
  loaded = $state(false);
  unavailable = $state<{ id: string; message: string }[]>([]);
  pending = $state.raw<StartBuildVideoArgs | null>(null);
  recoveryError = $state('');
  audioRecovery = $state<{ jobId: string; childId: string; sessionId: string; applied: boolean } | null>(null);
  private after: string | null = null;
  private alive = true;
  private uncertain = false;
  constructor(private api: Pick<HostApi, 'session'>, private journal?: StartJournal) { this.restore(); }
  restore() {
    if (this.busy || this.pending) return;
    try { this.pending = this.journal?.read() ?? null; this.uncertain = !!this.pending; this.recoveryError = ''; }
    catch (e) { this.recoveryError = String(e); }
  }
  private clearPending() {
    if (this.pending) this.journal?.clear(this.pending);
    this.pending = null;
  }
  dispose() { this.alive = false; }
  private async run(action: () => Promise<void>) {
    if (!this.alive || this.busy || this.recoveryError) return;
    this.busy = true; this.error = '';
    try { await action(); }
    catch (e) { if (this.alive) this.error = e && typeof e === 'object' && 'message' in e ? String(e.message) : String(e); }
    finally { if (this.alive) this.busy = false; }
  }
  private async job(request: BuildVideoRequest, id: string) {
    const response = await this.api.session({ type: 'build_video', request });
    if (response.type !== 'build_video_job' || response.job.job_id !== id) throw new Error(extraText("制作の応答が一致しません。"));
    return response.job;
  }
  private accept(job: BuildVideoJob) {
    if (!this.alive) return;
    this.unavailable = this.unavailable.filter(entry => entry.id !== job.job_id);
    this.selected = job;
    this.jobs = this.jobs.some(old => old.job_id === job.job_id)
      ? this.jobs.map(old => old.job_id === job.job_id ? job : old) : [job, ...this.jobs];
  }
  async start(input: Omit<StartBuildVideoArgs, 'job_id'>) {
    if (this.pending) return;
    await this.run(async () => {
      // Capture the entire recipe; later form edits must not alter a retry.
      const args = JSON.parse(JSON.stringify({ ...input, job_id: crypto.randomUUID() })) as StartBuildVideoArgs;
      try { this.journal?.write(args); } catch (e) { this.recoveryError = String(e); throw e; }
      this.pending = args;
      this.uncertain = false;
      await this.submit();
    });
  }
  private async submit() {
    const args = this.pending;
    if (!args) return;
    this.journal?.verify(args);
    try {
      const job = await this.job({ type: 'start', args }, args.job_id);
      if (!this.alive) return;
      this.clearPending(); this.accept(job);
    } catch (e) {
      if (!this.alive) return;
      const details = e && typeof e === 'object' && 'details' in e ? e.details as Record<string, string> : null;
      if (!this.uncertain && details?.job_created === 'false' && details.job_id === args.job_id) this.clearPending();
      else this.uncertain = true;
      throw e;
    }
  }
  async retry() { if (this.pending) await this.run(() => this.submit()); }
  async checkPending() {
    const args = this.pending;
    if (!args) return;
    await this.run(async () => {
      const job = await this.job({ type: 'get', job_id: args.job_id, wait_ms: 0 }, args.job_id);
      if (!this.alive) return;
      this.accept(job);
      // Queued may mean persisted but not launched (host busy). Keep identical
      // Start available; observing a queued record does not dispatch its work.
      if (job.phase !== 'queued') this.clearPending();
    });
  }
  async refresh(id = this.selected?.job_id) {
    if (this.pending) return;
    if (id) await this.run(async () => this.accept(await this.job({ type: 'get', job_id: id, wait_ms: 0 }, id)));
  }
  async reconcileAudio() {
    const selected = this.selected;
    if (!selected || this.pending || selected.stale || selected.phase !== 'interrupted') return;
    const prior = this.audioRecovery?.jobId === selected.job_id ? this.audioRecovery : null;
    if (prior?.applied) return;
    const childId = prior?.childId ?? selected.child_job_id;
    if (!childId || (!prior && !recoverySpeechStep(selected))) return;
    await this.run(async () => {
      // Keep the exact target after an ambiguous response, even if a later Get
      // shows that the reservation was cleared. Retrying must never pick a new step.
      const target = prior ?? { jobId: selected.job_id, childId, sessionId: selected.session_id, applied: false };
      this.audioRecovery = target;
      const result = await this.job({ type: 'reconcile_audio', args: {
        job_id: target.jobId, child_job_id: target.childId,
      } }, target.jobId);
      if (result.session_id !== target.sessionId || result.phase !== 'interrupted' || result.child_job_id !== null)
        throw new Error(locale.text('buildAudioRecoveryMismatch'));
      if (!this.alive) return;
      this.accept(result);
      this.audioRecovery = { ...target, applied: true };
    });
  }
  async cancel() {
    const job = this.selected;
    if (!job || !buildActive(job) || (this.pending && this.pending.job_id !== job.job_id)) return;
    await this.run(async () => {
      const cancelled = await this.job({ type: 'cancel', job_id: job.job_id }, job.job_id);
      if (!this.alive) return;
      this.accept(cancelled);
      if (this.pending?.job_id === cancelled.job_id && !['queued', 'running'].includes(cancelled.phase)) this.clearPending();
    });
  }
  async list(next = false) {
    if (this.pending || (next && !this.more)) return;
    await this.run(async () => {
      const response = await this.api.session({ type: 'build_video', request: { type: 'list', after: next ? this.after : null, limit: 12 } });
      if (!this.alive) return;
      if (response.type !== 'build_video_jobs' || response.job_ids.length > 12 || new Set(response.job_ids).size !== response.job_ids.length) throw new Error(extraText("制作一覧の応答が不正です。"));
      const jobs: BuildVideoJob[] = [];
      const unavailable: { id: string; message: string }[] = [];
      for (const id of response.job_ids) {
        if (!this.alive) return;
        try { jobs.push(await this.job({ type: 'get', job_id: id, wait_ms: 0 }, id)); }
        catch (e) { unavailable.push({ id, message: e && typeof e === 'object' && 'message' in e ? String(e.message) : String(e) }); }
      }
      if (!this.alive) return;
      this.jobs = next ? [...this.jobs, ...jobs.filter(job => !this.jobs.some(old => old.job_id === job.job_id))] : jobs;
      this.unavailable = next
        ? [...this.unavailable.filter(old => !response.job_ids.includes(old.id)), ...unavailable]
        : unavailable;
      this.after = response.job_ids.at(-1) ?? null; this.more = response.job_ids.length === 12; this.loaded = true;
      if (this.selected) this.selected = jobs.find(job => job.job_id === this.selected?.job_id) ?? this.selected;
    });
  }
}
