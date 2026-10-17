import { locale } from '../../shared/i18n/locale.svelte';
import type { HostApi, ProxyJob, ProxyRequest, SessionSnapshot, GenerateProxiesArgs } from '@sungeki/api-client';
export const proxyActive = (job: ProxyJob) => ['queued', 'running', 'cancelling'].includes(job.phase);
const phaseKeys = { queued: 'proxyQueued', running: 'proxyRunning', cancelling: 'proxyCancelling', cancelled: 'proxyCancelled', succeeded: 'proxySucceeded', failed: 'proxyFailed', interrupted: 'proxyInterrupted' } as const;
export const proxyPhase = (phase: ProxyJob['phase']) => locale.text(phaseKeys[phase]);

export class ProxyJobsController {
  jobs = $state<ProxyJob[]>([]);
  selected = $state<ProxyJob | null>(null);
  pending = $state.raw<GenerateProxiesArgs | null>(null);
  busy = $state(false);
  error = $state('');
  notice = $state('');
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
  private async job(request: ProxyRequest, id: string) {
    const response = await this.api.session({ type: 'proxy', request });
    if (response.type !== 'proxy_job' || response.job.session_id !== this.sessionId || response.job.job_id !== id) {
      throw new Error(locale.text('proxyMismatch'));
    }
    return response.job;
  }
  private accept(job: ProxyJob) {
    if (!this.alive) return;
    this.selected = job;
    this.jobs = [job, ...this.jobs.filter(old => old.job_id !== job.job_id)];
  }
  async start(snapshot: SessionSnapshot, assetIds: string[], preset: 'compact' | 'standard') {
    if (this.pending || snapshot.session_id !== this.sessionId) return;
    if (!assetIds.length || assetIds.length > 64 || new Set(assetIds).size !== assetIds.length || assetIds.some(id => {
      const asset = snapshot.project.assets.find(a => a.id === id);
      return asset?.probe.type !== 'video' || asset.locator.type !== 'absolute';
    })) { this.error = locale.text('proxyInvalidTargets'); return; }
    await this.run(async () => {
      this.pending = { session_id: this.sessionId, job_id: crypto.randomUUID(), expected_revision: snapshot.revision, asset_ids: [...assetIds], preset };
      await this.submit();
    });
  }
  private async submit() {
    const args = this.pending;
    if (!args) return;
    try {
      const job = await this.job({ type: 'start', args }, args.job_id);
      if (!this.alive) return;
      if (JSON.stringify(job.asset_ids) !== JSON.stringify(args.asset_ids) || job.max_edge !== (args.preset === 'compact' ? 640 : 1280) || job.source_revision !== args.expected_revision) throw new Error(locale.text('proxyTargetMismatch'));
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
  async remove(assetId: string): Promise<boolean> {
    const job = this.selected;
    if (!job || this.pending || proxyActive(job) || !job.asset_ids.includes(assetId)) return false;
    let succeeded = false;
    await this.run(async () => {
      this.notice = '';
      const response = await this.api.session({ type: 'proxy', request: { type: 'remove', args: { job_id: job.job_id, asset_id: assetId } } });
      if (!this.alive) return;
      if (response.type !== 'proxy_removed' || response.job_id !== job.job_id || response.asset_id !== assetId) throw new Error(locale.text('proxyRemoveMismatch'));
      succeeded = true;
      this.notice = locale.text('proxyRemoved');
    });
    return succeeded;
  }
  async list(next = false) {
    if (this.pending) return;
    await this.run(async () => {
      const response = await this.api.session({ type: 'proxy', request: {
        type: 'list', session_id: this.sessionId, after: next ? this.after : null, limit: 12,
      } });
      if (!this.alive) return;
      if (response.type !== 'proxy_jobs' || response.job_ids.length > 12) throw new Error(locale.text('proxyInvalidList'));
      const jobs: ProxyJob[] = [];
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
