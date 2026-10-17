import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'svelte/server';
import type { FramePreviewJob } from '@sungeki/api-client';
import type { EditorController } from '../editor/controller.svelte';
import { locale } from '../../shared/i18n/locale.svelte';
import { FramePreviewJobsController, previewPhase } from './jobs.svelte';
import { PublicationController } from './publication.svelte';
import { frameText, frameStage } from './messages';
import FramePreview from './FramePreview.svelte';
import PublishOutput from './PublishOutput.svelte';

afterEach(() => { locale.current = 'ja'; });

test('image entry and uncertain-save guidance translate while preserving the destination', () => {
  const editor = { snapshot: { session_id: 's' } } as EditorController;
  const publication = new PublicationController({ session: vi.fn() }, 's');
  publication.pending = { job_id: 'j', path: '/tmp/日本語$&.png' };
  const job = { job_id: 'j' } as FramePreviewJob;
  locale.current = 'en';
  expect(render(FramePreview, { props: { editor } }).body).toContain('Preview final image');
  const body = render(PublishOutput, { props: { publication, job } }).body;
  expect(body).toContain('The save result is unknown.');
  expect(body).toContain('Check the same destination');
  expect(body).toContain('Different files with the same name will not be overwritten.');
  expect(publication.pending.path).toBe('/tmp/日本語$&.png');
  locale.current = 'ja';
  expect(render(FramePreview, { props: { editor } }).body).toContain('正式な画像を確認');
  expect(render(PublishOutput, { props: { publication, job } }).body).toContain('同じ保存先で再確認');
});

test('phases and known stages follow locale, and unknown diagnostics are preserved', async () => {
  locale.current = 'en';
  const phases: FramePreviewJob['phase'][] = ['queued', 'running', 'cancelling', 'cancelled', 'succeeded', 'failed', 'interrupted'];
  expect(phases.map(previewPhase)).toEqual(['Queued', 'Generating image', 'Cancelling', 'Cancelled', 'Image generated', 'Failed', 'Interrupted']);
  expect(frameStage('確認画像の検証完了')).toBe('Image verification complete');
  expect(frameStage('unknown worker stage')).toBe('unknown worker stage');
  expect(frameText('画像-{id}.png', { id: '$&日本語' })).toBe('image-$&日本語.png');
  const session = vi.fn().mockResolvedValue({ type: 'frame_preview_jobs', job_ids: [] });
  const jobs = new FramePreviewJobsController({ session }, 's');
  await jobs.refresh('j');
  expect(jobs.error).toBe('The image preview response does not match the request.');
  expect(jobs.selected).toBeNull();
  locale.current = 'ja';
  expect(previewPhase('failed')).toBe('失敗');
  await jobs.refresh('j');
  expect(jobs.error).toBe('確認画像の応答が一致しません。');
});
