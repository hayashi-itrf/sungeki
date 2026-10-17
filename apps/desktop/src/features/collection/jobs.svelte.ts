import { collectionText as t } from './messages';
import type { HostApi, CollectionJob, CollectionRequest, SessionSnapshot, CollectProjectArgs, CollectionPlanPage } from '@sungeki/api-client';
export const collectionActive = (job: CollectionJob) => ['queued', 'running', 'cancelling'].includes(job.phase);
export const collectionPhase = { queued: '待機中', running: '確認・収集中', cancelling: '取消処理中', cancelled: '取消済み', succeeded: '完了', failed: '失敗', interrupted: '中断' } as const;

export class CollectionJobsController {
  jobs = $state<CollectionJob[]>([]);
  selected = $state<CollectionJob | null>(null);
  pending = $state.raw<CollectProjectArgs | null>(null);
  busy = $state(false);
  error = $state('');
  more = $state(false);
  page = $state<CollectionPlanPage | null>(null);
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
  private async job(request: CollectionRequest, id: string) {
    const response = await this.api.session({ type: 'collection', request });
    if (response.type !== 'collection_job' || response.job.session_id !== this.sessionId || response.job.job_id !== id) {
      throw new Error(t("素材収集の応答が一致しません。"));
    }
    return response.job;
  }
  private accept(job: CollectionJob) {
    if (!this.alive) return;
    if (this.selected?.job_id !== job.job_id) this.page = null;
    this.selected = job;
    this.jobs = [job, ...this.jobs.filter(old => old.job_id !== job.job_id)];
  }
  async start(snapshot: SessionSnapshot, destination: string, dryRun: boolean, includeCheckpoints = false) {
    if (this.pending || snapshot.session_id !== this.sessionId) return;
    await this.run(async () => {
      this.pending = { session_id: this.sessionId, job_id: crypto.randomUUID(), expected_revision: snapshot.revision, destination, dry_run: dryRun, include_checkpoints: includeCheckpoints };
      await this.submit();
    });
  }
  private async submit() {
    const args = this.pending;
    if (!args) return;
    try {
      const job = await this.job({ type: 'start', args }, args.job_id);
      if (job.include_checkpoints !== args.include_checkpoints || job.dry_run !== args.dry_run || job.source_revision !== args.expected_revision) throw new Error(t("素材収集の条件が一致しません。"));
      if (!this.alive) return;
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
      const response = await this.api.session({ type: 'collection', request: {
        type: 'list', session_id: this.sessionId, after: next ? this.after : null, limit: 12,
      } });
      if (!this.alive) return;
      if (response.type !== 'collection_jobs' || response.job_ids.length > 12) throw new Error(t("素材収集の一覧が不正です。"));
      const jobs: CollectionJob[] = [];
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
  async loadPage(offset = 0) {
    const job = this.selected;
    if (!job?.plan || this.pending) return;
    await this.run(async () => {
      const response = await this.api.session({ type: 'collection', request: {
        type: 'read_plan', args: { job_id: job.job_id, offset, limit: 25 },
      } });
      if (!this.alive) return;
      if (response.type !== 'collection_plan_page' || response.page.offset !== offset ||
          response.page.total !== job.plan!.source_count || response.page.items.length > 25 ||
          response.page.items.length !== Math.min(25, response.page.total - offset)) throw new Error(t("素材一覧の応答が一致しません。"));
      this.page = response.page;
    });
  }

}
