import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'svelte/server';
import type { SessionResponse, SessionSnapshot } from '@sungeki/api-client';
import { locale } from '../../shared/i18n/locale.svelte';
import { SpeechJobsController, speechPhase } from './jobs.svelte';
import SpeechStartStatus from './SpeechStartStatus.svelte';

afterEach(() => { locale.current = 'ja'; });

test('pending start is informational; uncertainty and retry appear only after a failed response', async () => {
  let reject!: (error: unknown) => void;
  const session = vi.fn(() => new Promise<SessionResponse>((_, no) => { reject = no; }));
  const jobs = new SpeechJobsController({ session }, 'session');
  const snapshot = { session_id: 'session', revision: 3, project: {
    clips: [{ id: 'speech', content: { type: 'speech', character_id: 'speaker' } }],
    characters: [{ id: 'speaker', voice: { type: 'sbv2' } }],
  } } as SessionSnapshot;
  const start = jobs.start(snapshot, 'speech', '', 'model');
  expect(jobs.busy).toBe(true);
  const original = jobs.pending;
  expect(original).not.toBeNull();
  for (const language of ['ja', 'en'] as const) {
    locale.current = language;
    const { body } = render(SpeechStartStatus, { props: { busy: jobs.busy, retry: vi.fn() } });
    expect(body).toContain('role="status"');
    expect(body).toContain(locale.text('speechStartWaiting'));
    expect(body).toContain(locale.text('speechStartElapsed').replace('{seconds}', '0'));
    expect(body).not.toContain('role="alert"');
    expect(body).not.toContain('<button');
  }
  reject({ code: 'HOST_UNAVAILABLE', message: 'response lost' });
  await start;
  expect(jobs.busy).toBe(false);
  expect(jobs.pending).toBe(original);
  for (const language of ['ja', 'en'] as const) {
    locale.current = language;
    const { body } = render(SpeechStartStatus, { props: { busy: jobs.busy, retry: vi.fn() } });
    expect(body).toContain('role="alert"');
    expect(body).toContain(locale.text('speechStartUncertain'));
    expect(body).toContain(locale.text('speechStartRetry'));
    expect(body).not.toContain(locale.text('speechStartWaiting'));
  }
  const retry = jobs.retry();
  expect(jobs.busy).toBe(true);
  expect(jobs.pending).toBe(original);
  reject({ code: 'CONFLICT', message: 'changed input' });
  await retry;
  expect(jobs.pending).toBeNull();
  expect(jobs.busy).toBe(false);
});


test('existing phase labels follow locale changes without recreating the job controller', () => {
  const labels = speechPhase;
  locale.current = 'ja';
  expect(labels.running).toBe('準備・音声生成中');
  expect(labels.succeeded).toBe('音声を保存済み');
  locale.current = 'en';
  expect(labels.running).toBe('Preparing / generating speech');
  expect(labels.succeeded).toBe('Audio saved');
  expect(labels.cancelling).toBe('Cancelling');
  expect(labels.cancelled).toBe('Cancelled');
  expect(labels.interrupted).toBe('Interrupted');
  locale.current = 'ja';
  expect(labels.running).toBe('準備・音声生成中');
});
