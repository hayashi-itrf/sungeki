import { expect, test, vi } from 'vitest';
import type { SessionRequest, SessionResponse, SessionSnapshot, SpeechJob } from '@sungeki/api-client';
import { SpeechBatch } from './batch.svelte';
import { SpeechJobsController } from './jobs.svelte';
function setup(engine: 'voicevox' | 'sbv2' | 'irodori' = 'voicevox') {
  let revision = 1;
  let overlap = false, fail = false, uncertain = false;
  const snapshot = () => ({ session_id: 's', revision, project: {
    clips: ['later', 'first', 'done'].map((id, i) => ({ id, start_us: i === 0 ? 10e6 : 0,
      content: { type: 'speech', character_id: 'char', audio: id === 'done' ? {} : null } })),
    characters: [{ id: 'char', voice: engine !== 'voicevox' ? { type: engine } : { type: 'voicevox', engine_id: 'engine', core_version: 'core' } }],
  } } as SessionSnapshot);
  const records = new Map<string, SpeechJob>();
  const session = vi.fn(async (r: SessionRequest): Promise<SessionResponse> => {
    if (r.type === 'get') return { type: 'snapshot', snapshot: snapshot() };
    if (r.type === 'discover_voicevox') return { type: 'voicevox_engine', engine: {
      engine_id: 'engine', name: 'engine', version: 'v1', core_versions: ['core'],
    } };
    if (r.type !== 'speech_job') throw new Error('unexpected request');
    if ((r.request.type === 'start' || r.request.type === 'start_sbv2' || r.request.type === 'start_irodori')) {
      const a = r.request.args;
      const job = { job_id: a.job_id, session_id: 's', clip_id: a.clip_id,
        source_revision: a.expected_revision, phase: 'queued', stale: false,
        application: { type: 'not_applicable' }, engine_settlement_required: false,
      } as SpeechJob;
      records.set(a.job_id, job);
      if (uncertain) throw { code: 'HOST_UNAVAILABLE', message: 'uncertain' };
      return { type: 'speech_job', job };
    }
    if (r.request.type !== 'get') throw new Error('unexpected job request');
    const job = records.get(r.request.job_id)!;
    job.phase = fail ? 'failed' : 'succeeded';
    job.application = { type: 'applied', revision: ++revision, overlapping_clip_ids: overlap ? ['other'] : [] };
    return { type: 'speech_job', job };
  });
  const jobs = new SpeechJobsController({ session }, 's');
  const batch = new SpeechBatch({ session }, jobs);
  const starts = () => session.mock.calls.map(([r]) => r).filter(r => r.type === 'speech_job' && (r.request.type === 'start' || r.request.type === 'start_sbv2' || r.request.type === 'start_irodori'));
  return { batch, jobs, session, snapshot, starts, edit: () => revision++,
    overlap: () => { overlap = true; }, fail: () => { fail = true; }, uncertain: () => { uncertain = true; } };
}
test('orders only missing audio and follows applied revision without editing the Project in UI', async () => {
  const s = setup(); await s.batch.start(s.snapshot(), 'http://localhost:55021');
  expect(s.starts()[0]).toMatchObject({ request: { args: { clip_id: 'first', expected_revision: 1 } } });
  await s.batch.advance();
  expect(s.starts()[1]).toMatchObject({ request: { args: { clip_id: 'later', expected_revision: 2 } } });
  await s.batch.advance();
  expect(s.batch.completed).toBe(2); expect(s.batch.total).toBe(2); expect(s.batch.running).toBe(false);
  expect(s.starts()).toHaveLength(2);
  expect(s.session.mock.calls.some(([r]) => r.type === 'edit')).toBe(false);
});
test('failure or overlap stops before dispatching the following clip', async () => {
  for (const mode of ['fail', 'overlap'] as const) {
    const s = setup(); s[mode](); await s.batch.start(s.snapshot(), 'http://localhost:55021');
    await s.batch.advance(); expect(s.batch.running).toBe(false); expect(s.starts()).toHaveLength(1);
  }
});
test('transient observation failure retains the same job and resumes the batch once observed', async () => {
  const s = setup(); await s.batch.start(s.snapshot(), 'http://localhost:55021');
  const id = s.jobs.selected!.job_id;
  for (let i = 0; i < 2; i++) {
    s.session.mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: 'temporarily unavailable' });
    await s.batch.advance();
    expect(s.batch.running).toBe(true); expect(s.batch.completed).toBe(0);
    expect(s.starts()).toHaveLength(1); expect(s.jobs.selected!.job_id).toBe(id);
    expect(s.session.mock.lastCall![0]).toEqual({ type: 'speech_job', request: { type: 'get', job_id: id, wait_ms: 0 } });
  }
  await s.batch.advance();
  expect(s.batch.completed).toBe(1); expect(s.starts()).toHaveLength(2);
  expect(s.jobs.errorCode).toBe(''); expect(s.jobs.error).toBe('');
  await s.batch.advance();
  expect(s.batch.completed).toBe(2); expect(s.batch.running).toBe(false);
  expect(s.starts()).toHaveLength(2);
});
test('non-transient read errors and explicit stop during an outage do not dispatch another clip', async () => {
  for (const code of ['NOT_FOUND', 'PROTOCOL_MISMATCH', 'HOST_UNAVAILABLE']) {
    const s = setup(); await s.batch.start(s.snapshot(), 'http://localhost:55021');
    s.session.mockRejectedValueOnce({ code, message: 'read failed' });
    await s.batch.advance();
    if (code === 'HOST_UNAVAILABLE') s.batch.stop();
    expect(s.batch.running).toBe(false);
    await s.batch.advance(); expect(s.starts()).toHaveLength(1);
    expect(s.batch.completed).toBe(0);
  }
});
test('uncertain start retains original request for explicit retry and never starts the next one', async () => {
  const s = setup(); s.uncertain(); await s.batch.start(s.snapshot(), 'http://localhost:55021');
  expect(s.batch.running).toBe(false); expect(s.jobs.pending).not.toBeNull();
  await s.batch.advance(); await s.batch.start(s.snapshot(), 'http://localhost:55021');
  expect(s.starts()).toHaveLength(1);
});
test('an edit before synthesis application also stops the following dispatch', async () => {
  const s = setup(); await s.batch.start(s.snapshot(), 'http://localhost:55021');
  s.edit(); await s.batch.advance();
  expect(s.batch.completed).toBe(1); expect(s.batch.running).toBe(false);
  expect(s.starts()).toHaveLength(1);
});
test('stop during snapshot fetch and externally changed revision prevent dispatch', async () => {
  for (const stop of [true, false]) {
    const s = setup(); const response = s.snapshot();
    let resolve!: (r: SessionResponse) => void;
    s.session.mockImplementationOnce(() => new Promise(r => { resolve = r; }));
    const work = s.batch.start(response, 'http://localhost:55021');
    if (stop) s.batch.stop(); else s.edit();
    resolve({ type: 'snapshot', snapshot: s.snapshot() }); await work;
    expect(s.batch.running).toBe(false); expect(s.starts()).toHaveLength(0);
  }
});


test('SBV2 batch retains its profile for each missing clip without VOICEVOX discovery', async () => {
  const s = setup('sbv2');
  await s.batch.start(s.snapshot(), 'unused', 'local-jp');
  await s.batch.advance();
  await s.batch.advance();
  expect(s.batch.completed).toBe(2);
  expect(s.starts()).toHaveLength(2);
  for (const request of s.starts()) expect(request).toMatchObject({ request: { type: 'start_sbv2', args: { profile_id: 'local-jp' } } });
  expect(s.session.mock.calls.some(([request]) => request.type === 'discover_voicevox')).toBe(false);
});

test('Irodori batch retains its profile for each missing clip without VOICEVOX discovery', async () => {
  const s = setup('irodori');
  await s.batch.start(s.snapshot(), 'unused', 'sbv2-setting', 'qwen-setting', 'local-jp');
  await s.batch.advance();
  await s.batch.advance();
  expect(s.batch.completed).toBe(2);
  expect(s.starts()).toHaveLength(2);
  for (const request of s.starts()) expect(request).toMatchObject({ request: { type: 'start_irodori', args: { profile_id: 'local-jp' } } });
  expect(s.session.mock.calls.some(([request]) => request.type === 'discover_voicevox')).toBe(false);
});
