import type { Locale } from '../../shared/i18n/locale.svelte';

// Presentation only: job state, cancellation and completion use typed phases.
// Keep unknown host descriptions visible rather than substituting a guessed stage.
const english: Readonly<Record<string, string>> = {
  '原本固定・映像音声を出力中': 'Freezing source media and exporting video and audio',
  '素材を確認・準備中': 'Checking and preparing source media',
  '音声とBGMを調整中': 'Mixing speech and background music',
  '映像と音声を動画ファイルにまとめています': 'Combining video and audio into the output file',
  '完成した動画の形式と内容を検査中': 'Verifying the completed video file',
  '映像・音声の出力検証完了': 'Video and audio export verification complete',
};

export function renderProgressLabel(stage: string, language: Locale): string {
  if (language === 'ja') return stage;
  const known = Object.hasOwn(english, stage) ? english[stage] : undefined;
  if (known) return known;
  const drawing = /^映像を作成中（([0-9]+) \/ ([0-9]+) コマ）$/.exec(stage);
  return drawing ? `Rendering frames (${drawing[1]} / ${drawing[2]})` : stage;
}
