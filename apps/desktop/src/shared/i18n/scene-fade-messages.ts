export const sceneFadeEnglish = {
  sceneFadeTitle: 'Whole-scene fade',
  sceneFadeHint: 'Fade the complete picture, including background, text and characters, from and to the canvas background color. Images outside the scene are included. Voice and BGM levels stay unchanged.',
  sceneFadeDurationHint: 'Set both durations to zero to remove the fade. Their sum must fit within the scene duration. This does not crossfade adjacent scenes.',
  sceneFadeStale: 'The project has changed. Load the latest state before continuing.',
  sceneFadeScene: 'Scene',
  sceneFadeDuration: '{name} ({seconds} seconds)',
  sceneFadeIn: 'Fade in (seconds)',
  sceneFadeOut: 'Fade out (seconds)',
  sceneFadePreview: 'Review changes',
  sceneFadeOperation: 'Fade the whole scene in from the background color over {in} seconds and out to it over {out} seconds',
  sceneFadeRemove: 'Remove the whole-scene fade',
} as const;

export const sceneFadeJapanese: Record<keyof typeof sceneFadeEnglish, string> = {
  sceneFadeTitle: 'シーン全体のフェード',
  sceneFadeHint: '背景・文字・立ち絵を重ねた画面全体を、作品の背景色から表示し、背景色へ戻します。シーンに所属しない画像も対象です。音声とBGMの音量は変わりません。',
  sceneFadeDurationHint: '両方を0秒にすると解除します。合計はシーンの長さ以内にしてください。隣の場面を重ねるクロスフェードではありません。',
  sceneFadeStale: '作品が更新されました。最新の状態を読み込んでください。',
  sceneFadeScene: 'シーン',
  sceneFadeDuration: '{name}（{seconds}秒）',
  sceneFadeIn: '表示までの長さ（秒）',
  sceneFadeOut: '消えるまでの長さ（秒）',
  sceneFadePreview: '変更内容を確認',
  sceneFadeOperation: 'シーン全体を背景色から{in}秒で表示し、{out}秒で背景色へ戻す',
  sceneFadeRemove: 'シーン全体のフェードを解除する',
};
