import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'svelte/server';
import type { SessionSnapshot } from '@sungeki/api-client';
import RenderError from './RenderError.svelte';
import backend from '../../../../../crates/host/src/render_location.rs?raw';
import { locale } from '../../shared/i18n/locale.svelte';
import { renderDiagnostic } from './render-diagnostic';

afterEach(() => { locale.current = 'ja'; });
import { RenderJobsController } from './jobs.svelte';

test('storage rejection exposes recovery actions, releases pending start and clears old advice on retry', async () => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'CAPABILITY_UNAVAILABLE', message: '保存先を確認',
    details: { job_created: 'false' }, suggested_actions: ['SSDを接続する', '保存先を修復する'] })
    .mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: '接続できません' });
  const jobs = new RenderJobsController({ session }, 's');
  const snapshot = { session_id: 's', revision: 7 } as SessionSnapshot;
  await jobs.start(snapshot);
  expect(jobs.pending).toBeNull();
  expect(jobs.errorActions).toEqual(['SSDを接続する', '保存先を修復する']);
  const html = render(RenderError, { props: { message: jobs.error, actions: jobs.errorActions } }).body;
  expect(html).toContain('role="alert"');
  expect(html).toContain('SSDを接続する');
  expect(html).toContain('保存先を修復する');
  await jobs.start(snapshot);
  expect(jobs.pending).not.toBeNull();
  expect(jobs.errorActions).toEqual([]);
  expect(jobs.error).toBe('接続できません');
});

test('diagnostic text stays escaped and an empty error produces no stale actions', () => {
  const html = render(RenderError, { props: { message: '<img src=x>', actions: ['<script>bad</script>'] } }).body;
  expect(html).toContain('&lt;img');
  expect(html).toContain('&lt;script>');
  expect(html).not.toContain('<script>');
  expect(render(RenderError, { props: { message: '', actions: ['old'] } }).body).not.toContain('old');
});


test('current workspace recovery guidance translates completely and retains restart advice', () => {
  const guidance = [...backend.split('pub(crate) fn root')[0].matchAll(/"([^"\n]+)"/g)]
    .map(match => match[1]).filter(text => /[ぁ-んァ-ヶ一-龠]/.test(text));
  expect(guidance).toHaveLength(4);
  const props = { message: guidance[0], actions: guidance.slice(1) };
  for (const text of guidance) expect(renderDiagnostic(text)).toBe(text);
  locale.current = 'en';
  const html = render(RenderError, { props }).body;
  expect(html).not.toMatch(/[ぁ-んァ-ヶ一-龠]/);
  expect(html).toContain('write permissions');
  expect(html).toContain('restart the host first');
  expect((html.match(/<li\b/g) ?? [])).toHaveLength(3);
  locale.current = 'ja';
  for (const text of guidance) expect(render(RenderError, { props }).body).toContain(text);
});

test('English mode preserves and escapes unfamiliar diagnostics and added context', () => {
  locale.current = 'en';
  const message = '新しい診断 <img src=x>';
  expect(renderDiagnostic(message)).toBe(message);
  expect(renderDiagnostic('__proto__')).toBe('__proto__');
  const html = render(RenderError, { props: { message, actions: ['<script>未知の対処</script>'] } }).body;
  expect(html).toContain('新しい診断');
  expect(html).toContain('&lt;img');
  expect(html).toContain('&lt;script>');
  expect(html).not.toContain('<script>');
});
