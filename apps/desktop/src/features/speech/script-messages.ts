import { locale } from '../../shared/i18n/locale.svelte';

const english = {
  "＋ 台本": "+ Script",
  "台本からセリフを追加": "Add speech from a script",
  "入力を取り消して閉じる": "Discard input and close",
  "閉じる": "Close",
  "開始・仮の長さ・間隔を確認してください。": "Check the start, temporary duration, and gap.",
  "セリフを入力してください。": "Enter speech text.",
  "1行を1つのセリフとして、新しい音声トラックに追加します。空行は飛ばします。話者名やMarkdownの記号も本文として扱います。": "Each nonblank line becomes a speech clip on a new audio track. Speaker names and Markdown symbols are treated as spoken text.",
  "先に「＋ キャラクター」から声を登録してください。": "Register a voice with “＋ Character” first.",
  "標準の話者": "Default speaker",
  "台本（空行を含め200行・100,000文字まで）": "Script (up to 200 lines including blank lines, and 100,000 characters)",
  "入力内容は切り詰めずに保持しています。次の上限を超えているため追加できません。": "Your full input is preserved. Reduce it to the limits below before adding it.",
  "行ごとの話者を確認してください。台本を修正する場合は、下のボタンで割当を解除して戻ります。": "Check the speaker for each line. To edit the script, clear the assignments with the button below.",
  "台本を書き直す（話者の割当を解除）": "Edit script (clear speaker assignments)",
  "行ごとに話者を割り当てる": "Assign speakers by line",
  "割り当てない場合は、すべて標準の話者で追加します。": "Without assignments, all lines use the default speaker.",
  "開始（秒）": "Start (seconds)",
  "1行の仮の長さ（秒）": "Temporary duration per line (seconds)",
  "セリフ間の間隔（秒）": "Gap between lines (seconds)",
  "所属シーン": "Scene",
  "シーンなし": "No scene",
  "音声はまだ生成しません。仮の長さで配置し、既存のクリップは動かしません。シーンを選ぶ場合は、その区間内に収めてください。": "Audio is not generated yet. Clips use temporary durations; existing clips stay in place. Keep the clips within the selected scene.",
  "「音声合成」で生成後、「音声の長さで並べる」で配置を調整できます。追加全体は「元に戻す」1回で取り消せます。": "After speech synthesis, use “Arrange by speech duration” to adjust placement. One Undo removes the entire addition.",
  "現在 {lines} / 200行（空行を含む）・{characters} / 100,000文字（改行を含む）": "{lines} / 200 lines (including blank lines) · {characters} / 100,000 characters (including line breaks)",
  "行数を{count}行以上減らし、200行以内にしてください。": "Remove at least {count} lines to stay within 200 lines.",
  "文字数を{count}文字以上減らし、100,000文字以内にしてください。": "Remove at least {count} characters to stay within 100,000 characters.",
  "{line}行目：": "Line {line}: ",
  "{line}行目の話者": "Speaker for line {line}",
  "{count}件のセリフを追加": "Add {count} speech clips",
  "こんにちは。\n次のセリフです。": "Hello.\nHere is the next line."
} as const;

export function scriptText(message: keyof typeof english, values: Record<string, string | number> = {}, language = locale.current): string {
  const text = language === 'en' ? english[message] : message;
  return text.replace(/\{(\w+)\}/g, (placeholder, key: string) => key in values ? String(values[key]) : placeholder);
}
