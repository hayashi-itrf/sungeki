export const speechLayoutEnglish = {
  "speechLayoutTitle": "Arrange by speech duration",
  "speechLayoutDescription": "Arrange selected lines in their current start-time order using generated audio durations and the specified gap. One Undo restores the previous arrangement.",
  "speechLayoutScope": "Backgrounds, characters, BGM and scene durations keep their current timing. Check their coverage after arranging speech. Audio is not regenerated.",
  "speechLayoutStale": "The project has changed. Load the latest state and try again.",
  "speechLayoutRefresh": "Discard input and load the latest state",
  "speechLayoutLines": "Lines to arrange (start-time order)",
  "speechLayoutMissing": " (audio not generated)",
  "speechLayoutStart": "Start position (seconds)",
  "speechLayoutGap": "Gap between lines (seconds)",
  "speechLayoutReview": "Review changes ({count} lines)",
  "speechLayoutAll": "Select all generated lines",
  "speechLayoutClear": "Clear selection",
  "speechLayoutLimit": "Select 1–200 lines per operation. Reduce the selection to 200 lines or fewer.",
  "speechLayoutEmpty": "There are no speech lines to arrange. Add speech first.",
  "speechLayoutGenerate": "No lines have generated audio. Synthesize speech first.",
  "speechLayoutCount": "{count} lines selected (maximum 200)"
} as const;

export const speechLayoutJapanese: Record<keyof typeof speechLayoutEnglish, string> = {
  "speechLayoutTitle": "音声の長さで並べる",
  "speechLayoutDescription": "選んだセリフを現在の開始時刻順に、生成済み音声の長さと指定した間隔で並べます。一度の「元に戻す」で戻せます。",
  "speechLayoutScope": "背景・立ち絵・BGM・シーンの長さは変わりません。並べた後に表示範囲を確認してください。音声の再生成は行いません。",
  "speechLayoutStale": "作品が更新されました。最新の状態を読み込んでからやり直してください。",
  "speechLayoutRefresh": "入力を破棄して最新の状態を読む",
  "speechLayoutLines": "並べるセリフ（開始時刻順）",
  "speechLayoutMissing": "（音声未生成）",
  "speechLayoutStart": "開始位置（秒）",
  "speechLayoutGap": "セリフ間の間隔（秒）",
  "speechLayoutReview": "変更内容を確認（{count}件）",
  "speechLayoutAll": "生成済みのセリフをすべて選択",
  "speechLayoutClear": "選択を解除",
  "speechLayoutLimit": "一度に並べられるのは1〜200件です。200件以下になるよう選択してください。",
  "speechLayoutEmpty": "並べられるセリフがありません。先にセリフを追加してください。",
  "speechLayoutGenerate": "音声が未生成です。先に音声合成を実行してください。",
  "speechLayoutCount": "{count}件を選択（上限200件）"
};
