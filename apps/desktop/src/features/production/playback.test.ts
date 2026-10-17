import { expect, test, vi } from 'vitest';
import type { BuildVideoJob, RenderJob, SessionResponse } from '@sungeki/api-client';
import { BuildPlayback } from './playback.svelte';
const output = { path: '/saved/video.mp4', sha256: 'hash', byte_length: 300, frame_count: 60, font_sha256: 'font', layout_version: 9 };
const build = { session_id: 'build-session', output: { render_job_id: 'render-id', render: output } } as Pick<BuildVideoJob, 'session_id' | 'output'>;
const job: RenderJob = { job_id: 'render-id', session_id: 'build-session', purpose: 'final', source_revision: 22,
  content_hash: 'project', phase: 'succeeded', output, error: null, progress: null, stale: true };
const reply = (changes: Partial<RenderJob> = {}): SessionResponse => ({ type: 'render_job', job: { ...job, ...changes } });
test('retrieves the exact child output, retaining current staleness and original revision', async () => {
  const session = vi.fn().mockResolvedValue(reply());
  const playback = new BuildPlayback({ session });
  await playback.load(build);
  expect(session).toHaveBeenCalledExactlyOnceWith({ type: 'render_job', request: { type: 'get', job_id: 'render-id', wait_ms: 0 } });
  expect(playback.job).toEqual(job);
});
test.each([
  { job_id: 'other' }, { session_id: 'other' }, { purpose: 'editing_preview' },
  { phase: 'running' }, { output: null }, { output: { ...output, sha256: 'changed' } },
  { output: { ...output, path: '/other.mp4' } }, { output: { ...output, layout_version: 8 } },
] as Partial<RenderJob>[] )('rejects a different or unfinished child output: %j', async changes => {
  const playback = new BuildPlayback({ session: vi.fn().mockResolvedValue(reply(changes)) });
  await playback.load(build);
  expect(playback.job).toBeNull(); expect(playback.error).not.toBe('');
});
test('a failed refresh removes the old player; read-only retry can recover', async () => {
  const session = vi.fn().mockResolvedValueOnce(reply()).mockRejectedValueOnce(new Error('offline')).mockResolvedValueOnce(reply());
  const playback = new BuildPlayback({ session });
  await playback.load(build); await playback.load(build);
  expect(playback.job).toBeNull(); expect(playback.error).toBe('offline');
  await playback.load(build); expect(playback.job).toEqual(job); expect(playback.error).toBe('');
});
test('changing selection disposes the old reader and ignores its late reply', async () => {
  let resolve!: (r: SessionResponse) => void;
  const session = vi.fn(() => new Promise<SessionResponse>(r => { resolve = r; }));
  const playback = new BuildPlayback({ session });
  const pending = playback.load(build); await playback.load(build);
  playback.dispose(); resolve(reply()); await pending; await playback.load(build);
  expect(session).toHaveBeenCalledTimes(1); expect(playback.job).toBeNull();
});

test('the verified build child can be saved through the common publication controller', async () => {
  const { PublicationController } = await import('../export/publication.svelte');
  const session = vi.fn().mockResolvedValueOnce(reply()).mockResolvedValueOnce({
    type: 'render_published', publication: {
      job_id: job.job_id, session_id: job.session_id, source_revision: job.source_revision,
      content_hash: job.content_hash, output: { ...output, path: '/chosen/movie.mp4' },
      already_present: false, stale: true,
    },
  });
  const playback = new BuildPlayback({ session });
  await playback.load(build);
  const publication = new PublicationController({ session }, build.session_id);
  await publication.choose(playback.job!, async () => '/chosen/movie.mp4');
  expect(session.mock.lastCall![0]).toEqual({ type: 'render_job', request: {
    type: 'publish', args: { job_id: 'render-id', path: '/chosen/movie.mp4' },
  } });
  expect(publication.result).toMatchObject({ stale: true, output: { path: '/chosen/movie.mp4', sha256: output.sha256 } });
  expect(playback.job!.output!.path).toBe('/saved/video.mp4');
});
