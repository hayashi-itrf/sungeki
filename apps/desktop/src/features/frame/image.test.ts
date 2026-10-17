import { expect, test, vi } from 'vitest';
import type { FramePreviewJob, SessionResponse } from '@sungeki/api-client';
import { FrameImage } from './image.svelte';

async function fixture() {
  const bytes = new Uint8Array(270000).fill(41);
  const hash = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes)), b => b.toString(16).padStart(2, '0')).join('');
  const job = { job_id: 'j', phase: 'succeeded', output: { byte_length: bytes.length, sha256: hash } } as FramePreviewJob;
  const response = (offset: number, length: number): SessionResponse => ({ type: 'frame_preview_bytes', data: {
    offset, total: bytes.length, base64: btoa(Array.from(bytes.slice(offset, offset + length), c => String.fromCharCode(c)).join('')),
  } });
  return { job, response };
}

test('joins bounded chunks, verifies the complete hash and releases the displayed image', async () => {
  const { job, response } = await fixture();
  const session = vi.fn(async r => response(r.request.args.offset, r.request.args.length));
  const image = new FrameImage({ session });
  await image.load(job);
  expect(image.error).toBe(''); expect(image.url).toMatch(/^blob:/); expect(session).toHaveBeenCalledTimes(2);
  const url = image.url, revoke = vi.spyOn(URL, 'revokeObjectURL');
  image.clear(); expect(revoke).toHaveBeenCalledWith(url); expect(image.url).toBe(''); revoke.mockRestore();
});

test('wrong whole-image hash or inconsistent range never produces a visible image', async () => {
  const { job, response } = await fixture();
  const session = vi.fn(async r => response(r.request.args.offset, r.request.args.length));
  const image = new FrameImage({ session });
  await image.load({ ...job, output: { ...job.output!, sha256: '0'.repeat(64) } });
  expect(image.url).toBe(''); expect(image.error).toContain('一致しません');
  session.mockImplementation(async () => response(12, 1));
  await image.load(job); expect(image.url).toBe(''); expect(image.error).toContain('応答');
});

test('closing during a read ignores its late result and does not request more chunks', async () => {
  const { job, response } = await fixture();
  let resolve!: (value: SessionResponse) => void;
  const session = vi.fn(() => new Promise<SessionResponse>(r => { resolve = r; }));
  const image = new FrameImage({ session });
  const loading = image.load(job);
  image.clear(); resolve(response(0, 262144)); await loading;
  expect(image.url).toBe(''); expect(image.busy).toBe(false); expect(session).toHaveBeenCalledTimes(1);
});
