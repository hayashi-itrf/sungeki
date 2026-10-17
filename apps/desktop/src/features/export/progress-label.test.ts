import { expect, test } from 'vitest';
import { renderProgressLabel } from './progress-label';

test('language changes preserve the observed frame counts without estimating completion', () => {
  const stage = '映像を作成中（128 / 900 コマ）';
  expect(renderProgressLabel(stage, 'en')).toBe('Rendering frames (128 / 900)');
  expect(renderProgressLabel(stage, 'ja')).toBe(stage);
  expect(renderProgressLabel('音声とBGMを調整中', 'en')).toBe('Mixing speech and background music');
});

test('unknown or incomplete observations remain visible, never labelled complete', () => {
  for (const stage of ['toString', '素材を検証中: custom', '映像を作成中（128 / ? コマ）', '']) {
    expect(renderProgressLabel(stage, 'en')).toBe(stage);
  }
  expect(renderProgressLabel('映像・音声の出力検証完了', 'en')).toBe('Video and audio export verification complete');
});
