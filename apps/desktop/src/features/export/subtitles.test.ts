import { expect, test, vi } from 'vitest';
import type { ExportSubtitlesArgs, SessionResponse } from '@sungeki/api-client';
import { SubtitlesController } from './subtitles.svelte';
const input = { session_id: 's', expected_revision: 2, format: 'srt', include_text: false } as const;
function reply(overrides = {}): SessionResponse {
  return { type: 'subtitles_exported', publication: { session_id: 's', source_revision: 2,
    format: 'srt', include_text: false, path: '/canonical/字幕.srt', sha256: 'a'.repeat(64),
    content_hash: 'b'.repeat(64), byte_length: 42, already_present: false, stale: false, ...overrides } };
}

test('chooser cancellation and disposal do not start a write', async () => {
  const session = vi.fn(), c = new SubtitlesController({ session }, 's');
  await c.choose(input, async () => null);
  expect(c.busy).toBe(false); expect(c.pending).toBeNull();
  let resolve!: (p: string) => void;
  const task = c.choose(input, () => new Promise<string>(r => { resolve = r; }));
  c.dispose(); resolve('/字幕.srt'); await task;
  expect(session).not.toHaveBeenCalled();
});

test('captures options before chooser, blocks duplicates, and accepts canonical response', async () => {
  const session = vi.fn().mockResolvedValue(reply()), c = new SubtitlesController({ session }, 's');
  const options: Omit<ExportSubtitlesArgs, 'path'> = { ...input };
  let resolve!: (p: string) => void;
  const task = c.choose(options, () => new Promise<string>(r => { resolve = r; }));
  options.format = 'web_vtt'; options.expected_revision = 3;
  const duplicate = vi.fn(); await c.choose(input, duplicate);
  expect(duplicate).not.toHaveBeenCalled();
  resolve('/alias/字幕.srt'); await task;
  expect(session).toHaveBeenCalledWith({ type: 'export_subtitles', args: { ...input, path: '/alias/字幕.srt' } });
  expect(c.result?.path).toBe('/canonical/字幕.srt'); expect(c.pending).toBeNull();
});

test('uncertain result retries exact request; later revision conflict keeps uncertainty until explicit dismissal', async () => {
  const session = vi.fn().mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: 'timeout' })
    .mockRejectedValueOnce({ code: 'CONFLICT', message: '作品が更新されています' }).mockResolvedValue(reply());
  const c = new SubtitlesController({ session }, 's');
  await c.choose(input, async () => '/字幕.srt'); const pending = c.pending;
  await c.retry(); expect(session.mock.lastCall![0].args).toBe(pending);
  expect(c.pending).toBe(pending); expect(c.result).toBeNull();
  await c.choose(input, async () => '/別.srt'); expect(session).toHaveBeenCalledTimes(2);
  c.dismiss(); expect(c.pending).toBeNull();
  await c.choose(input, async () => '/別.srt'); expect(c.result).not.toBeNull();
});

test('mismatched responses remain unconfirmed and late responses are ignored after disposal', async () => {
  for (const changes of [{ session_id: 'other' }, { source_revision: 3 }, { format: 'web_vtt' },
    { include_text: true }, { path: 'relative.srt' }, { sha256: 'bad' }, { byte_length: -1 }]) {
    const c = new SubtitlesController({ session: vi.fn().mockResolvedValue(reply(changes)) }, 's');
    await c.choose(input, async () => '/字幕.srt');
    expect(c.result).toBeNull(); expect(c.pending).not.toBeNull(); expect(c.error).toContain('一致しません');
  }
  let resolve!: (r: SessionResponse) => void;
  const c = new SubtitlesController({ session: vi.fn(() => new Promise<SessionResponse>(r => { resolve = r; })) }, 's');
  const task = c.choose(input, async () => '/字幕.srt');
  await Promise.resolve(); c.dispose(); resolve(reply()); await task;
  expect(c.result).toBeNull();
});

test('successful retry identifies existing and stale subtitles', async () => {
  const session = vi.fn().mockRejectedValueOnce(new Error('応答なし')).mockResolvedValue(reply({ already_present: true, stale: true }));
  const c = new SubtitlesController({ session }, 's');
  await c.choose(input, async () => '/字幕.srt'); await c.retry();
  expect(c.result).toMatchObject({ already_present: true, stale: true });
  expect(c.error).toBe(''); expect(c.pending).toBeNull();
});
