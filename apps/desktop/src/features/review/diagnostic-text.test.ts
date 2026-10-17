import { afterEach, expect, test } from 'vitest';
import structure from '../../../../../crates/review/src/structure.rs?raw';
import images from '../../../../../crates/review/src/images.rs?raw';
import audio_timing from '../../../../../crates/review/src/audio_timing.rs?raw';
import project_images from '../../../../../crates/review/src/project_images.rs?raw';
import representative_frames from '../../../../../crates/review/src/representative_frames.rs?raw';
import text_inspection from '../../../../../crates/review/src/text_inspection.rs?raw';
import { locale } from '../../shared/i18n/locale.svelte';
import { checkDescription, issueDescription } from './diagnostic-text';

afterEach(() => { locale.current = 'ja'; });


test('current backend fixed check descriptions are covered without changing Japanese', () => {
  const texts = [structure, images, audio_timing, project_images, representative_frames, text_inspection]
    .flatMap(source => [...source.matchAll(/(?:description:\s*|description\s*=\s*|\.push_str\()"([^"\n]+)"/g)].map(match => match[1].trim()));
  expect(texts.length).toBeGreaterThan(12);
  for (const text of texts) {
    expect(checkDescription(text)).toBe(text);
    locale.current = 'en';
    // Additional asset notes follow the image description in real reports.
    const combined = /^(PSD|動画は|VRM\/VRMA)/.test(text)
      ? `登録された画像はありません（検査対象なし）。 ${text}` : text;
    expect(checkDescription(combined)).not.toMatch(/[ぁ-んァ-ヶ一-龠]/);
    locale.current = 'ja';
  }
});

test('partial image generation retains every coverage count and never claims full inspection', () => {
  locale.current = 'en';
  const text = '代表画像7枚中2枚を生成。候補の省略11枚。画像を抜き出していない区間13か所。画像一覧3ページ中1ページ生成。全編の検査ではありません。';
  expect(checkDescription(text)).toBe('Generated 2 of 7 representative images. Omitted candidates: 11. Unsampled intervals: 13. Generated 1 of 3 contact sheet pages. This is not an inspection of the full video.');
  expect(checkDescription(text + '新しい診断')).toBe(text + '新しい診断');
});

test('text inspection retains partial coverage, overflow, margin settings and uninspected scope', () => {
  locale.current = 'en';
  const prefix = '文字の全17描画区間中12区間を正式フォント・描画処理で検査し、3区間ではみ出しを検出。';
  const suffix = ' 重なり・読みやすさは未検査です。';
  const result = checkDescription(prefix + '上下左右5%の安全余白に4区間の文字が入っています。画面/配置枠のはみ出し区間は安全余白の追加検査を省略しています。' + suffix);
  expect(result).toContain('12 of 17');
  expect(result).toContain('overflow found in 3');
  expect(result).toContain('4 intervals enters the 5%');
  expect(result).toContain('checks were skipped');
  expect(result).toContain('Overlap and readability remain uninspected');
  expect(checkDescription(prefix + '安全余白は未設定・未検査です。' + suffix)).toContain('not configured and remain uninspected');
});

test('unknown diagnostics and extended details are preserved verbatim, with no report mutation', () => {
  const issue = { code: 'ASSET_MISSING', message: '画像ファイルが見つかりません' };
  expect(issueDescription(issue)).toBe(issue.message);
  locale.current = 'en';
  expect(issueDescription(issue)).toBe('Image file not found.');
  expect(issue.message).toBe('画像ファイルが見つかりません');
  const detailed = { ...issue, message: `${issue.message}: /日本語/$&.png` };
  expect(issueDescription(detailed)).toBe(detailed.message);
  expect(issueDescription({ ...issue, code: 'NEW_CODE' })).toBe(issue.message);
  expect(checkDescription('未知の診断: /日本語/$&.png')).toBe('未知の診断: /日本語/$&.png');
});
