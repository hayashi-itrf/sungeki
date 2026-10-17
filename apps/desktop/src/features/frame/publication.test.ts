import { expect, test, vi } from 'vitest';
import type { FramePreviewJob, SessionResponse } from '@sungeki/api-client';
import { PublicationController } from './publication.svelte';
const job: FramePreviewJob = { job_id: 'j', session_id: 's', source_revision: 2, content_hash: 'project',
  phase: 'succeeded', progress: null, error: null, stale: false,
  output: { path: '/work/output.png', sha256: 'video', byte_length: 123, frame_index: 30, time_us: 1000000, width: 1920, height: 1080, font_sha256: 'font', layout_version: 1 } };
function reply(): SessionResponse {
  return { type: 'frame_preview_published', publication: { job_id: job.job_id, session_id: job.session_id,
    source_revision: job.source_revision, content_hash: job.content_hash, output: { ...job.output!, path: '/canonical/movie.png' },
    already_present: true, stale: true } };
}

test('dialog cancellation and non-successful jobs never copy', async () => {
  const session = vi.fn(), select = vi.fn().mockResolvedValue(null);
  const p = new PublicationController({ session }, 's');
  await p.choose({ ...job, phase: 'failed', output: null }, select);
  expect(select).not.toHaveBeenCalled();
  await p.choose(job, select);
  expect(select).toHaveBeenCalledTimes(1); expect(session).not.toHaveBeenCalled();
  expect(p.pending).toBeNull(); expect(p.busy).toBe(false);
});

test('uncertain copy retains the exact request, rejects duplicate clicks, then accepts canonical path and stale replay', async () => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: 'timeout' }).mockResolvedValue(reply());
  const p = new PublicationController({ session }, 's');
  await p.choose(job, async () => '/alias/movie.png');
  const pending = p.pending;
  expect(pending).toEqual({ job_id: 'j', path: '/alias/movie.png' });
  await p.choose(job, async () => '/other.png'); expect(session).toHaveBeenCalledTimes(1);
  await p.retry(); expect(session.mock.lastCall![0].request.args).toBe(pending);
  expect(p.pending).toBeNull(); expect(p.result).toMatchObject({ already_present: true, stale: true, output: { path: '/canonical/movie.png' } });
  expect(job.output!.path).toBe('/work/output.png');
});

test('explicit rejection permits choosing another destination, while durability uncertainty retains retry', async () => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'OUTPUT_EXISTS', message: 'exists' })
    .mockRejectedValueOnce({ code: 'IO_ERROR', message: 'sync uncertain', details: { publication: 'durability_uncertain' } });
  const p = new PublicationController({ session }, 's');
  await p.choose(job, async () => '/a.png'); expect(p.pending).toBeNull(); expect(p.error).toBe('exists');
  await p.choose(job, async () => '/b.png'); expect(p.pending?.path).toBe('/b.png');
  expect(p.error).toBe('sync uncertain');
});

test('mismatched publication never claims success and disposal ignores a late result', async () => {
  const wrong = reply(); if (wrong.type === 'frame_preview_published') wrong.publication.output.sha256 = 'wrong';
  const session = vi.fn().mockResolvedValueOnce(wrong);
  const p = new PublicationController({ session }, 's');
  await p.choose(job, async () => '/a.png');
  expect(p.result).toBeNull(); expect(p.pending).not.toBeNull(); expect(p.error).toContain('一致しません');
  let resolve!: (r: SessionResponse) => void;
  session.mockImplementation(() => new Promise<SessionResponse>(r => { resolve = r; }));
  const retry = p.retry(); p.dispose(); resolve(reply()); await retry;
  expect(p.result).toBeNull(); await p.retry(); expect(session).toHaveBeenCalledTimes(2);
});

test('closing the native chooser after disposal cannot start a copy', async () => {
  const session = vi.fn(); const p = new PublicationController({ session }, 's');
  let resolve!: (path: string) => void;
  const selecting = p.choose(job, () => new Promise<string>(r => { resolve = r; }));
  await p.choose(job, async () => '/duplicate.png');
  p.dispose(); resolve('/chosen.png'); await selecting;
  expect(session).not.toHaveBeenCalled();
});

test('saving status and unresolved-result instructions are displayed separately', async () => {
  const { render } = await import('svelte/server');
  const { default: PublishOutput } = await import('./PublishOutput.svelte');
  const p = new PublicationController({ session: vi.fn() }, 's');
  p.pending = { job_id: 'j', path: '/movie.png' }; p.busy = true;
  let html = render(PublishOutput, { props: { publication: p, job } }).body;
  expect(html).toContain('画像の確認と保存中'); expect(html).not.toContain('保存結果が未確認');
  p.busy = false;
  html = render(PublishOutput, { props: { publication: p, job } }).body;
  expect(html).toContain('保存結果が未確認'); expect(html).toContain('同じ保存先で再確認');
});
