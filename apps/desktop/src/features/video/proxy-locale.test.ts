import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'svelte/server';
import type { SessionSnapshot } from '@sungeki/api-client';
import type { EditorController } from '../editor/controller.svelte';
import { locale } from '../../shared/i18n/locale.svelte';
import { ProxySelection } from './proxy-selection.svelte';
import { ProxyJobsController, proxyPhase } from './proxy-jobs.svelte';
import { proxyStage } from './proxy-labels';
import ProxyPanel from './ProxyPanel.svelte';

afterEach(() => { locale.current = 'ja'; });
const snapshot = { session_id: 's', revision: 2, project: { clips: [], assets: [
  { id: 'a', name: '日本語の素材名', probe: { type: 'video' }, locator: { type: 'absolute' } },
] } } as unknown as SessionSnapshot;

test('proxy panel translates controls and original-only export guidance without changing asset names', () => {
  const before = structuredClone(snapshot);
  const props = { editor: { snapshot, selection: [] } as unknown as EditorController, selection: new ProxySelection() };
  locale.current = 'en';
  const english = render(ProxyPanel, { props }).body;
  for (const text of ['Create lightweight videos', 'Generate selected video', 'Generate all project videos',
    'Original selected', 'Audio previews and final exports use originals.', 'No generation history yet.', '日本語の素材名']) {
    expect(english).toContain(text);
  }
  expect(english).not.toContain('動画を軽量化');
  locale.current = 'ja';
  const japanese = render(ProxyPanel, { props }).body;
  expect(japanese).toContain('動画を軽量化');
  expect(japanese).toContain('音声確認と正式出力は原本を使います。');
  expect(snapshot).toEqual(before);
});

test('proxy phases and known stages follow locale changes and preserve unknown diagnostics', () => {
  locale.current = 'en';
  expect(['queued', 'running', 'cancelling', 'cancelled', 'succeeded', 'failed', 'interrupted']
    .map(phase => proxyPhase(phase as Parameters<typeof proxyPhase>[0])))
    .toEqual(['Queued', 'Generating', 'Cancelling', 'Cancelled', 'Generated', 'Failed', 'Interrupted']);
  expect(proxyStage('軽量動画の検証完了')).toBe('Lightweight video verification complete');
  expect(proxyStage('書き出しなどの処理を待っています')).toBe('Waiting for exports and other foreground work');
  expect(proxyStage('unknown diagnostic')).toBe('unknown diagnostic');
  locale.current = 'ja';
  expect(proxyPhase('cancelled')).toBe('取消済み');
  expect(proxyStage('軽量動画の検証完了')).toBe('軽量動画の検証完了');
});

test('proxy request validation and mismatched response errors use the selected language', async () => {
  locale.current = 'en';
  const session = vi.fn().mockResolvedValue({ type: 'proxy_jobs', job_ids: [] });
  const jobs = new ProxyJobsController({ session }, 's');
  await jobs.start(snapshot, [], 'compact');
  expect(jobs.error).toContain('Select 1–64 unique video assets');
  expect(session).not.toHaveBeenCalled();
  await jobs.refresh('job');
  expect(jobs.error).toBe('The lightweight video response does not match the request.');
  locale.current = 'ja';
  await jobs.refresh('job');
  expect(jobs.error).toContain('軽量動画生成の応答が一致しません');
});
