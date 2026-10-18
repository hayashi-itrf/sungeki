import { localEndpoint } from './local-connection';
import { locale } from '../../shared/i18n/locale.svelte';
import type { HostApi, SessionRequest, SessionSnapshot, SpeechJob, StartSpeechArgs, StartSbv2SpeechArgs } from '@sungeki/api-client';
export const speechActive = (job: SpeechJob) => ['queued', 'running', 'cancelling'].includes(job.phase);
export const speechPhase = {
  get queued() { return locale.text('speechPhaseQueued'); },
  get running() { return locale.text('speechPhaseRunning'); },
  get cancelling() { return locale.text('speechPhaseCancelling'); },
  get cancelled() { return locale.text('speechPhaseCancelled'); },
  get succeeded() { return locale.text('speechPhaseSucceeded'); },
  get failed() { return locale.text('speechPhaseFailed'); },
  get interrupted() { return locale.text('speechPhaseInterrupted'); },
};
export class SpeechJobsController {
  jobs = $state<SpeechJob[]>([]);
  selected = $state<SpeechJob | null>(null);
  pending = $state.raw<StartSpeechArgs | StartSbv2SpeechArgs | null>(null);
  busy = $state(false);
  error = $state('');
  errorCode = $state('');
  more = $state(false);
  private pendingLocal = false;
  private pendingEngine: 'start_sbv2' | 'start_qwen' | 'start_irodori' = 'start_sbv2';
  private after: string | null = null;
  private alive = true;
  constructor(private api: Pick<HostApi, 'session'>, readonly sessionId: string) {}
  dispose() { this.alive = false; }
  private async run(action: () => Promise<void>) {
    if (this.busy || !this.alive) return;
    this.busy = true; this.error = ''; this.errorCode = '';
    try { await action(); }
    catch (e) {
      if (this.alive) {
        this.error = e && typeof e === 'object' && 'message' in e ? String(e.message) : String(e);
        this.errorCode = e && typeof e === 'object' && 'code' in e ? String(e.code) : '';
      }
    }
    finally { if (this.alive) this.busy = false; }
  }
  private async job(request: SessionRequest, id: string) {
    const response = await this.api.session(request);
    if (response.type !== 'speech_job' || response.job.session_id !== this.sessionId || response.job.job_id !== id) throw new Error(locale.text('speechResponseMismatch'));
    return response.job;
  }
  private accept(job: SpeechJob) {
    if (!this.alive) return;
    this.selected = job;
    this.jobs = [job, ...this.jobs.filter(old => old.job_id !== job.job_id)];
  }
  async start(snapshot: SessionSnapshot, clipId: string, baseUrl: string, profileId = '', qwenProfileId = '', irodoriProfileId = '') {
    if (this.pending || snapshot.session_id !== this.sessionId) return;
    const clip = snapshot.project.clips.find(c => c.id === clipId);
    const content = clip?.content;
    if (content?.type !== 'speech') return;
    const character = snapshot.project.characters.find(c => c.id === content.character_id);
    if (!character) return;
    const voice = character.voice;
    await this.run(async () => {
      this.pendingLocal = false;
      if (voice.type === 'local_http') {
        const base_url = localEndpoint(voice.settings.engine);
        const response = await this.api.session({ type: 'get_local_speech_catalog', args: { base_url, engine: voice.settings.engine } });
        if (!this.alive) return;
        if (response.type !== 'local_speech_catalog' || response.catalog.engine !== voice.settings.engine || !response.catalog.speakers.some(s => s.speaker_uuid === voice.settings.speaker_uuid && s.styles.some(s => s.id === voice.settings.style_id))) throw new Error(locale.text('speechEngineMismatch'));
        this.pendingLocal = true;
        this.pending = { session_id: this.sessionId, job_id: crypto.randomUUID(), expected_revision: snapshot.revision, clip_id: clipId, base_url, engine_version: response.catalog.version };
        await this.submit(); return;
      }
      if (voice.type === 'sbv2' || voice.type === 'qwen' || voice.type === 'irodori') {
        const profile = (voice.type === 'irodori' ? irodoriProfileId : voice.type === 'qwen' ? qwenProfileId : profileId).trim();
        this.pendingEngine = voice.type === 'irodori' ? 'start_irodori' : voice.type === 'qwen' ? 'start_qwen' : 'start_sbv2';
        if (!/^[A-Za-z0-9_-]{1,64}$/.test(profile)) throw new Error(locale.text('speechProfileRequired').replace('{engine}', { qwen: 'Qwen', sbv2: 'SBV2', irodori: 'Irodori' }[voice.type]));
        this.pending = { session_id: this.sessionId, job_id: crypto.randomUUID(), expected_revision: snapshot.revision, clip_id: clipId, profile_id: profile };
        await this.submit();
        return;
      }
      const response = await this.api.session({ type: 'discover_voicevox', args: { base_url: baseUrl } });
      if (!this.alive) return;
      if (response.type !== 'voicevox_engine') throw new Error(locale.text('speechEngineResponseInvalid'));
      if (response.engine.engine_id !== voice.engine_id || !response.engine.core_versions.includes(voice.core_version)) throw new Error(locale.text('speechEngineMismatch'));
      this.pending = { session_id: this.sessionId, job_id: crypto.randomUUID(), expected_revision: snapshot.revision, clip_id: clipId, base_url: baseUrl, engine_version: response.engine.version };
      await this.submit();
    });
  }
  private async submit() {
    const args = this.pending;
    if (!args) return;
    try {
      const request: SessionRequest = { type: 'speech_job', request: 'profile_id' in args ? { type: this.pendingEngine, args } : { type: this.pendingLocal ? 'start_local' : 'start', args } };
      const job = await this.job(request, args.job_id);
      if (!this.alive) return;
      if (job.source_revision !== args.expected_revision || job.clip_id !== args.clip_id) throw new Error(locale.text('speechResponseMismatch'));
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
    await this.run(async () => this.accept(await this.job({ type: 'speech_job', request: { type: 'get', job_id: id, wait_ms: 0 } }, id)));
  }
  async cancel() {
    const id = this.selected?.job_id;
    if (!id || this.pending) return;
    await this.run(async () => this.accept(await this.job({ type: 'speech_job', request: { type: 'cancel', job_id: id } }, id)));
  }
  async list(next = false) {
    if (this.pending) return;
    await this.run(async () => {
      const response = await this.api.session({ type: 'speech_job', request: { type: 'list', session_id: this.sessionId, after: next ? this.after : null, limit: 12 } });
      if (!this.alive) return;
      if (response.type !== 'speech_jobs' || response.job_ids.length > 12) throw new Error(locale.text('speechHistoryResponseInvalid'));
      const jobs: SpeechJob[] = [];
      for (const id of response.job_ids) {
        if (!this.alive) return;
        jobs.push(await this.job({ type: 'speech_job', request: { type: 'get', job_id: id, wait_ms: 0 } }, id));
      }
      if (!this.alive) return;
      this.jobs = next ? [...this.jobs, ...jobs.filter(job => !this.jobs.some(old => old.job_id === job.job_id))] : jobs;
      this.after = response.job_ids.at(-1) ?? null; this.more = response.job_ids.length === 12;
      if (this.selected) this.selected = jobs.find(job => job.job_id === this.selected?.job_id) ?? this.selected;
    });
  }
}
