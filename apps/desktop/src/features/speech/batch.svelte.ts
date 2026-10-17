import { locale } from '../../shared/i18n/locale.svelte';
import type { HostApi, SessionSnapshot } from '@sungeki/api-client';
import { SpeechJobsController, speechActive } from './jobs.svelte';

// UI scheduling only: synthesis, validation, persistence and application remain in the host.
export class SpeechBatch {
  running = $state(false);
  completed = $state(0);
  total = $state(0);
  message = $state('');
  private ids: string[] = [];
  private revision = 0;
  private url = '';
  private profile = '';
  private qwenProfile = '';
  private irodoriProfile = '';
  private current: string | null = null;
  private advancing = false;
  private generation = 0;
  constructor(private api: Pick<HostApi, 'session'>, private jobs: SpeechJobsController) {}

  async start(snapshot: SessionSnapshot, url: string, profileId = '', qwenProfileId = '', irodoriProfileId = '') {
    if (this.running || this.advancing || this.jobs.busy || this.jobs.pending
      || (this.jobs.selected && speechActive(this.jobs.selected))
      || snapshot.session_id !== this.jobs.sessionId) return;
    this.ids = snapshot.project.clips.filter(c => c.content.type === 'speech' && c.content.audio === null)
      .sort((a, b) => a.start_us - b.start_us).map(c => c.id);
    this.total = this.ids.length; this.completed = 0; this.message = '';
    if (!this.total) { this.message = locale.text('speechBatchEmpty'); return; }
    this.generation++; this.running = true; this.current = null;
    this.revision = snapshot.revision; this.url = url; this.profile = profileId; this.qwenProfile = qwenProfileId; this.irodoriProfile = irodoriProfileId;
    await this.advance();
  }
  stop(message = locale.text('speechBatchStopHint')) {
    this.generation++; this.running = false; this.message = message;
  }
  async advance() {
    if (!this.running || this.advancing || this.jobs.busy || this.jobs.pending) return;
    const generation = this.generation;
    this.advancing = true;
    try {
      if (this.current) {
        await this.jobs.refresh(this.current);
        if (!this.running || generation !== this.generation) return;
        // A failed read says nothing about synthesis completion. Keep the same job.
        if (this.jobs.errorCode === 'HOST_UNAVAILABLE') return;
        if (this.jobs.error) throw new Error(this.jobs.error);
        const job = this.jobs.selected;
        if (!job || job.job_id !== this.current) throw new Error(locale.text('speechBatchMissingResult'));
        if (speechActive(job)) return;
        if (job.phase !== 'succeeded' || job.stale || job.application.type !== 'applied'
          || job.engine_settlement_required) throw new Error(locale.text('speechBatchNotApplied'));
        this.completed++;
        if (job.application.revision !== this.revision + 1) throw new Error(locale.text('speechBatchEditedDuring'));
        this.revision = job.application.revision;
        this.current = null;
        if (job.application.overlapping_clip_ids.length) throw new Error(locale.text('speechBatchOverlap'));
        if (this.completed === this.total) { this.stop(locale.text('speechBatchDone')); return; }
      }
      const response = await this.api.session({ type: 'get', session_id: this.jobs.sessionId });
      if (!this.running || generation !== this.generation) return;
      if (response.type !== 'snapshot' || response.snapshot.session_id !== this.jobs.sessionId
        || response.snapshot.revision !== this.revision) throw new Error(locale.text('speechBatchEdited'));
      await this.jobs.start(response.snapshot, this.ids[this.completed], this.url, this.profile, this.qwenProfile, this.irodoriProfile);
      if (!this.running || generation !== this.generation) return;
      if (this.jobs.pending) throw new Error(locale.text('speechBatchUncertain'));
      if (this.jobs.error) throw new Error(this.jobs.error);
      const job = this.jobs.selected;
      if (!job || job.clip_id !== this.ids[this.completed]) throw new Error(locale.text('speechBatchStartFailed'));
      this.current = job.job_id;
    } catch (error) {
      if (generation === this.generation) this.stop(error instanceof Error ? error.message : String(error));
    } finally { this.advancing = false; }
  }
}
