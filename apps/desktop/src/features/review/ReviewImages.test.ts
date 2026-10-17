import { expect, test } from 'vitest';
import { render } from 'svelte/server';
import type { ReviewFrames, FramePreviewOutput } from '@sungeki/api-client';
import ReviewImages from './ReviewImages.svelte';

test('offers only successful frames and sheets, exposes stale and uninspected status', () => {
  const output = { path: '/owned.png', sha256: 'hash', byte_length: 1 } as FramePreviewOutput;
  const frames: ReviewFrames = {
    frames: [
      { sample: { frame_index: 30, time_us: 1_000_000, reasons: [] }, output, error: null },
      { sample: { frame_index: 60, time_us: 2_000_000, reasons: [] }, output: null, error: 'failed' },
    ],
    contact_sheets: [{ page: 1, frame_indices: [30], output, error: null }],
    sampling: { frame_count: 100, selected: [], scenes_without_visible_frame: [], omitted: [{ frame_index: 99, time_us: 3_300_000, reasons: [] }], unsampled: [{ first: 1, end: 30 }] },
    check: { kind: 'representative_frames', status: 'failed', description: 'One image failed' },
    visual_review: { kind: 'visual_review', status: 'not_run', description: 'Not inspected' },
  };
  const html = render(ReviewImages, { props: { jobId: 'job', frames, stale: true } }).body;
  expect(html).toContain('一覧 1 ページ目');
  expect(html).toContain('1.000 秒の画像');
  expect(html).not.toContain('2.000 秒の画像');
  expect(html).toContain('作品・素材・描画処理の変更前の画像');
  expect(html).toContain('目視確認済みにはなりません');
  expect(html).not.toContain('<img'); // Reading requires explicit selection.
  expect(html).toContain('3.300000');
  expect(html).toContain('1 – 29'); // Half-open host range becomes inclusive for display.
  expect(html).toContain('この結果を作成した時点の範囲');
});
