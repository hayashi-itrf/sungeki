import { expect, test, vi } from 'vitest';
import type { SessionResponse, SessionSnapshot, SpeechJob } from '@sungeki/api-client';
import { SpeechJobsController } from './jobs.svelte';
const snapshot = { session_id: 's', revision: 7, project: {
  clips: [{ id: 'clip', content: { type: 'speech', gain_db: null, audio: null, character_id: 'char', subtitle_text: '字幕', spoken_text: null } }],
  characters: [{ id: 'char', voice: { type: 'voicevox', engine_id: 'engine', core_version: 'core' } }],
} } as SessionSnapshot;
const engine: SessionResponse = { type: 'voicevox_engine', engine: { engine_id: 'engine', name: 'engine', version: 'v1', core_versions: ['core'] } };
test('Irodori without a profile cannot fall through to VOICEVOX discovery', async () => {
  const input = structuredClone(snapshot);
  input.project.characters[0]!.voice = { type: 'irodori', settings: {
    voice_id: 'registered', caption: null, cfg_scale: 3, num_steps: 40, seed: 42,
  }, identity: { checkpoint: 'model', model_manifest_sha256: 'a'.repeat(64),
    codec_manifest_sha256: 'b'.repeat(64), engine_manifest_sha256: 'c'.repeat(64),
    voice_manifest_sha256: 'd'.repeat(64), sample_rate: 48000, channels: 1 } };
  const session = vi.fn();
  const jobs = new SpeechJobsController({ session }, 's');
  await jobs.start(input, 'clip', 'http://127.0.0.1:50021');
  expect(session).not.toHaveBeenCalled();
  expect(jobs.pending).toBeNull();
  expect(jobs.error).toContain('Irodori');
});
function result(id: string, overrides: Partial<SpeechJob> = {}): SessionResponse {
  return { type: 'speech_job', job: { job_id: id, session_id: 's', clip_id: 'clip', source_revision: 7, input_hash: 'hash', phase: 'queued', progress: null, engine_settlement_required: false, stale: false, artifact: null, application: { type: 'not_applicable' }, error: null, ...overrides } };
}
test('start discovers version and uses confirmed snapshot without editing Project', async () => {
  const session = vi.fn().mockResolvedValueOnce(engine).mockImplementation(async r => result(r.request.args.job_id));
  const jobs = new SpeechJobsController({ session }, 's');
  await jobs.start(snapshot, 'clip', 'http://127.0.0.1:50021');
  expect(session.mock.calls[1]![0].request.args).toMatchObject({ session_id: 's', expected_revision: 7, clip_id: 'clip', engine_version: 'v1' });
  expect(jobs.selected?.phase).toBe('queued'); expect(jobs.pending).toBeNull();
  expect(snapshot.revision).toBe(7);
});
test('uncertain start retains original ID, revision and endpoint; retry does not rediscover or create a new request', async () => {
  const session = vi.fn().mockResolvedValueOnce(engine).mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: '応答不明' });
  const jobs = new SpeechJobsController({ session }, 's');
  await jobs.start(snapshot, 'clip', 'http://127.0.0.1:50021');
  const original = jobs.pending;
  expect(original).not.toBeNull();
  await jobs.start({ ...snapshot, revision: 8 }, 'clip', 'http://127.0.0.1:50022');
  expect(session).toHaveBeenCalledTimes(2);
  session.mockImplementation(async r => result(r.request.args.job_id));
  await jobs.retry();
  expect(session.mock.lastCall![0].request.args).toBe(original);
  expect(jobs.selected?.job_id).toBe(original!.job_id); expect(jobs.pending).toBeNull();
});
test('engine/core mismatch and disposal during discovery cannot dispatch synthesis', async () => {
  const bad = new SpeechJobsController({ session: vi.fn().mockResolvedValue({ ...engine, engine: { engine_id: 'other', core_versions: ['core'] } }) }, 's');
  await bad.start(snapshot, 'clip', 'http://127.0.0.1:1');
  expect(bad.error).toContain('一致しません'); expect(bad.pending).toBeNull();
  let resolve!: (r: SessionResponse) => void;
  const session = vi.fn(() => new Promise<SessionResponse>(r => { resolve = r; }));
  const jobs = new SpeechJobsController({ session }, 's');
  const waiting = jobs.start(snapshot, 'clip', 'http://127.0.0.1:1');
  jobs.dispose(); resolve(engine); await waiting;
  expect(session).toHaveBeenCalledTimes(1); expect(jobs.pending).toBeNull();
});
test('wrong job response keeps the uncertain request; explicit conflict permits a fresh start', async () => {
  const session = vi.fn().mockResolvedValueOnce(engine).mockResolvedValueOnce(result('wrong'));
  const jobs = new SpeechJobsController({ session }, 's');
  await jobs.start(snapshot, 'clip', 'http://127.0.0.1:1');
  expect(jobs.error).toContain('一致しません'); expect(jobs.pending).not.toBeNull();
  expect(jobs.selected).toBeNull();
  session.mockRejectedValueOnce({ code: 'CONFLICT', message: '対象変更' });
  await jobs.retry(); expect(jobs.pending).toBeNull();
});
test('paging uses server cursor; cancel and refresh update one identified job', async () => {
  const ids = Array.from({ length: 12 }, (_, i) => `job-${i}`);
  const session = vi.fn(async r => r.request.type === 'list' ? { type: 'speech_jobs', job_ids: r.request.after ? ['next'] : ids } as SessionResponse : result(r.request.job_id, { phase: r.request.type === 'cancel' ? 'cancelled' : 'running' }));
  const jobs = new SpeechJobsController({ session }, 's');
  await jobs.list(); expect(jobs.jobs).toHaveLength(12); expect(jobs.more).toBe(true);
  await jobs.list(true); expect(jobs.jobs).toHaveLength(13); expect(jobs.more).toBe(false);
  expect(session.mock.calls[13]![0].request.after).toBe('job-11');
  await jobs.refresh('next'); await jobs.cancel();
  expect(jobs.selected?.phase).toBe('cancelled');
  expect(jobs.jobs.filter(j => j.job_id === 'next')).toHaveLength(1);
});


test('SBV2 requires a profile and never falls back to VOICEVOX discovery', async () => {
  const value = structuredClone(snapshot);
  value.project.characters[0]!.voice = { type: 'sbv2' } as typeof value.project.characters[0]['voice'];
  const session = vi.fn();
  const jobs = new SpeechJobsController({ session }, 's');
  await jobs.start(value, 'clip', 'http://127.0.0.1:50021');
  expect(jobs.error).toContain('SBV2');
  expect(session).not.toHaveBeenCalled();
  expect(jobs.pending).toBeNull();
});


test('SBV2 submits a captured profile and uncertain retry preserves the exact request', async () => {
  const value = structuredClone(snapshot);
  value.project.characters[0]!.voice = { type: 'sbv2' } as typeof value.project.characters[0]['voice'];
  const session = vi.fn().mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: '確認中' });
  const jobs = new SpeechJobsController({ session }, 's');
  await jobs.start(value, 'clip', 'unused-voicevox-url', ' local-jp ');
  expect(session).toHaveBeenCalledTimes(1);
  expect(session.mock.calls[0]![0]).toMatchObject({ type: 'speech_job', request: { type: 'start_sbv2', args: {
    session_id: 's', clip_id: 'clip', expected_revision: 7, profile_id: 'local-jp',
  } } });
  const captured = jobs.pending;
  await jobs.start({ ...value, revision: 8 }, 'clip', '', 'other');
  expect(session).toHaveBeenCalledTimes(1);
  session.mockImplementation(async r => result(r.request.args.job_id));
  await jobs.retry();
  expect(session.mock.lastCall![0].request.args).toBe(captured);
  expect(jobs.pending).toBeNull();
  expect(jobs.selected?.phase).toBe('queued');
  session.mockImplementation(async r => result(r.request.job_id, { phase: 'cancelled' }));
  await jobs.cancel();
  expect(session.mock.lastCall![0].request.type).toBe('cancel');
  expect(jobs.selected?.phase).toBe('cancelled');
});
test('invalid or missing SBV2 configuration cannot silently switch engines', async () => {
  const value = structuredClone(snapshot);
  value.project.characters[0]!.voice = { type: 'sbv2' } as typeof value.project.characters[0]['voice'];
  const session = vi.fn().mockRejectedValue({ code: 'CAPABILITY_UNAVAILABLE', message: 'Sungeki Maker用SBV2起動設定がありません' });
  const jobs = new SpeechJobsController({ session }, 's');
  await jobs.start(value, 'clip', '', '../outside');
  expect(session).not.toHaveBeenCalled();
  await jobs.start(value, 'clip', '', 'local-jp');
  expect(session).toHaveBeenCalledTimes(1);
  expect(jobs.error).toContain('起動設定がありません');
  expect(jobs.pending).toBeNull();
  expect(jobs.selected).toBeNull();
});
test('Qwen without a profile never falls through to VOICEVOX discovery', async () => {
  const value = structuredClone(snapshot);
  value.project.characters[0]!.voice = { type: 'qwen' } as typeof value.project.characters[0]['voice'];
  const session = vi.fn();
  const jobs = new SpeechJobsController({ session }, 's');
  await jobs.start(value, 'clip', 'http://127.0.0.1:50021');
  expect(session).not.toHaveBeenCalled();
  expect(jobs.pending).toBeNull();
  expect(jobs.error).toContain('Qwen');
});

test('Qwen submits a captured profile and uncertain retry preserves the exact request', async () => {
  const value = structuredClone(snapshot);
  value.project.characters[0]!.voice = { type: 'qwen' } as typeof value.project.characters[0]['voice'];
  const session = vi.fn().mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: '確認中' });
  const jobs = new SpeechJobsController({ session }, 's');
  await jobs.start(value, 'clip', 'unused-voicevox-url', 'sbv2-profile', ' local-jp ');
  expect(session).toHaveBeenCalledTimes(1);
  expect(session.mock.calls[0]![0]).toMatchObject({ type: 'speech_job', request: { type: 'start_qwen', args: {
    session_id: 's', clip_id: 'clip', expected_revision: 7, profile_id: 'local-jp',
  } } });
  const captured = jobs.pending;
  await jobs.start({ ...value, revision: 8 }, 'clip', '', 'other-sbv2', 'other-qwen');
  expect(session).toHaveBeenCalledTimes(1);
  session.mockImplementation(async r => result(r.request.args.job_id));
  await jobs.retry();
  expect(session.mock.lastCall![0].request.args).toBe(captured);
  expect(jobs.pending).toBeNull();
  expect(jobs.selected?.phase).toBe('queued');
  session.mockImplementation(async r => result(r.request.job_id, { phase: 'cancelled' }));
  await jobs.cancel();
  expect(session.mock.lastCall![0].request.type).toBe('cancel');
  expect(jobs.selected?.phase).toBe('cancelled');
});

test('Irodori submits a captured profile and uncertain retry preserves the exact request', async () => {
  const value = structuredClone(snapshot);
  value.project.characters[0]!.voice = { type: 'irodori' } as typeof value.project.characters[0]['voice'];
  const session = vi.fn().mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: '確認中' });
  const jobs = new SpeechJobsController({ session }, 's');
  await jobs.start(value, 'clip', 'unused-voicevox-url', 'sbv2-profile', 'qwen-profile', ' local-jp ');
  expect(session).toHaveBeenCalledTimes(1);
  expect(session.mock.calls[0]![0]).toMatchObject({ type: 'speech_job', request: { type: 'start_irodori', args: {
    session_id: 's', clip_id: 'clip', expected_revision: 7, profile_id: 'local-jp',
  } } });
  const captured = jobs.pending;
  await jobs.start({ ...value, revision: 8 }, 'clip', '', 'other-sbv2', 'other-qwen', 'other-irodori');
  expect(session).toHaveBeenCalledTimes(1);
  session.mockImplementation(async r => result(r.request.args.job_id));
  await jobs.retry();
  expect(session.mock.lastCall![0].request.args).toBe(captured);
  expect(jobs.pending).toBeNull();
  expect(jobs.selected?.phase).toBe('queued');
  session.mockImplementation(async r => result(r.request.job_id, { phase: 'cancelled' }));
  await jobs.cancel();
  expect(session.mock.lastCall![0].request.type).toBe('cancel');
  expect(jobs.selected?.phase).toBe('cancelled');
});

test.each([{ source_revision: 8 }, { clip_id: 'another-clip' }])('mismatched synthesis target %j cannot replace the pending request', async mismatch => {
  const session = vi.fn().mockResolvedValueOnce(engine).mockImplementation(async r => result(r.request.args.job_id, mismatch));
  const jobs = new SpeechJobsController({ session }, 's');
  await jobs.start(snapshot, 'clip', 'http://127.0.0.1:50021');
  const args = jobs.pending;
  expect(args).toMatchObject({ expected_revision: 7, clip_id: 'clip' });
  expect(jobs.selected).toBeNull(); expect(jobs.jobs).toEqual([]);
  expect(jobs.error).toContain('一致しません');
  await jobs.start({ ...snapshot, revision: 8 }, 'clip', 'http://127.0.0.1:50022');
  expect(session).toHaveBeenCalledTimes(2);
  session.mockImplementation(async r => result(r.request.args.job_id));
  await jobs.retry();
  expect(session.mock.lastCall![0].request.args).toBe(args);
  expect(jobs.pending).toBeNull(); expect(jobs.selected?.clip_id).toBe('clip');
});
