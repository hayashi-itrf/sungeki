import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'svelte/server';
import type { BuildVideoJob } from '@sungeki/api-client';
import { BuildJobsController } from './jobs.svelte';
import BuildAudioRecovery from './BuildAudioRecovery.svelte';
import { locale } from '../../shared/i18n/locale.svelte';
const base: BuildVideoJob = {
  job_id: 'parent', session_id: 'project', title: '中断作品', phase: 'interrupted',
  completed_steps: 2, total_steps: 9, progress: null, source_revision: 1,
  child_job_id: 'saved-audio', stale: false, output: null, review_asset_freshness: null, error: null,
};
const recovered = { ...base, completed_steps: 3, source_revision: 2, child_job_id: null };
afterEach(() => { locale.current = 'ja'; });

test('recovery targets the selected project and does not start another production', async () => {
  const session = vi.fn().mockResolvedValue({ type: 'build_video_job', job: recovered });
  const jobs = new BuildJobsController({ session }); jobs.selected = { ...base };
  await jobs.reconcileAudio();
  expect(session).toHaveBeenCalledExactlyOnceWith({ type: 'build_video', request: {
    type: 'reconcile_audio', args: { job_id: 'parent', child_job_id: 'saved-audio' },
  } });
  expect(jobs.selected).toEqual(recovered);
  expect(jobs.audioRecovery?.applied).toBe(true);
  await jobs.reconcileAudio(); expect(session).toHaveBeenCalledTimes(1);
  expect(render(BuildAudioRecovery, { props: { jobs } }).body).toContain('制作全体は中断のまま');
});

test('lost response keeps the original child target after a refresh clears its reservation', async () => {
  const session = vi.fn().mockRejectedValueOnce(new Error('connection lost'))
    .mockResolvedValue({ type: 'build_video_job', job: recovered });
  const jobs = new BuildJobsController({ session }); jobs.selected = { ...base };
  await jobs.reconcileAudio(); expect(jobs.audioRecovery?.applied).toBe(false);
  expect(jobs.selected).toEqual(base);
  await jobs.refresh(); await jobs.reconcileAudio();
  expect(session.mock.lastCall![0].request.args.child_job_id).toBe('saved-audio');
  expect(jobs.audioRecovery?.applied).toBe(true);
});

test.each([{ phase: 'running' as const }, { child_job_id: null }, { stale: true }, { completed_steps: 7 }])('does not recover an ineligible selection %o', change => {
  const session = vi.fn(); const jobs = new BuildJobsController({ session });
  jobs.selected = { ...base, ...change };
  void jobs.reconcileAudio(); expect(session).not.toHaveBeenCalled();
});

test('wrong session response never claims recovery succeeded', async () => {
  const session = vi.fn().mockResolvedValue({ type: 'build_video_job', job: { ...recovered, session_id: 'other' } });
  const jobs = new BuildJobsController({ session }); jobs.selected = { ...base };
  await jobs.reconcileAudio(); expect(jobs.error).toContain('一致しません');
  expect(jobs.selected).toEqual(base); expect(jobs.audioRecovery?.applied).toBe(false);
});

test.each(['ja', 'en'] as const)('recovery panel explains its limited scope and disables local draft conflicts: %s', language => {
  locale.current = language;
  const jobs = new BuildJobsController({ session: vi.fn() }); jobs.selected = { ...base };
  const html = render(BuildAudioRecovery, { props: { jobs, disabled: true } }).body;
  expect(html).toContain('disabled');
  expect(html).toContain(locale.text('buildAudioRecoveryScope'));
  jobs.selected = { ...base, phase: 'succeeded' };
  expect(render(BuildAudioRecovery, { props: { jobs } }).body).not.toContain('<aside');
});
