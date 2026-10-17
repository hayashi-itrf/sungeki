import { locale } from '../../shared/i18n/locale.svelte';

const english = {
  "正式な画像を確認": "Preview final image",
  "閉じる": "Close",
  "動画と同じ描画で1枚のPNGを作ります。字幕・立ち絵・配置の確認や画像の保存に使えます。音声の完成状態は検査しません。": "Generate a PNG using the same rendering as the final video. Check subtitles, characters and layout, or save the image. This does not check the completed audio.",
  "描画する位置": "Render position",
  "フレーム番号（先頭は0）": "Frame number (starts at 0)",
  "この位置の画像を作る": "Generate this frame",
  "画面を開いた時の再生位置を初期値にしています。作品の終端以降は指定できません。": "The initial value is the playback position when this panel opens. Choose a frame before the end of the project.",
  "開始結果が未確認です。": "The start result is unknown.",
  "同じ要求を再確認": "Check the same request",
  "確認画像の履歴": "Image preview history",
  "一覧を更新": "Refresh list",
  "確認画像はまだありません。": "No image previews yet.",
  "次の12件": "Next 12",
  "確認画像の結果": "Image preview result",
  "作品または描画処理が変更されたため古い画像です。現在の内容が必要な場合は作り直してください。": "This image is outdated because the project or renderer changed. Generate it again to see the current result.",
  "画像を読み込んで表示": "Load and display image",
  "画像を検査しています…": "Verifying image…",
  "正式描画による確認画像": "Image rendered with final output settings",
  "画像を表示できません。再生成してください。": "The image could not be displayed. Generate it again.",
  "状態を更新": "Refresh status",
  "生成を取り消す": "Cancel generation",
  "画面を閉じても生成処理は続きます。元の素材の外部変更は、画像を作り直すと検査されます。": "Generation continues when this panel closes. External changes to source assets are checked when you generate a new image.",
  "完成画像の保存先": "Save completed image",
  "PNG画像": "PNG image",
  "完成画像を別の場所へ保存": "Save completed image elsewhere",
  "保存先を選んで保存": "Choose destination and save",
  "完成画像を確認してからコピーします。同名の別ファイルは上書きしません。": "The completed image is verified before copying. Different files with the same name will not be overwritten.",
  "保存先の選択・画像の確認と保存中…": "Choosing a destination, verifying and saving the image…",
  "保存結果が未確認です。同じ保存先で再確認できます。": "The save result is unknown. Check again with the same destination.",
  "確認する保存先": "Destination to check",
  "同じ保存先で再確認": "Check the same destination",
  "アプリを終了する場合は保存先を控えてください。再起動後に同じ出力と保存先を選ぶと、重複せず確認できます。": "Keep a record of the destination before closing the app. After restarting, select the same output and destination to check without duplicating the file.",
  "同じ内容の画像が保存されていることを確認しました。": "An identical image is already saved.",
  "指定した場所へ保存しました。": "Saved to the selected destination.",
  "保存した画像": "Saved image",
  "保存開始時点で、作品または描画処理の変更前に作成された画像です。": "When saving started, this image was already from an earlier version of the project or renderer.",
  "保存時に完成原本との一致を確認済みです。画像内容の目視確認は別途必要です。": "The saved file was verified against the completed original. Visual review of the image is still required.",
  "確認画像の応答が一致しません。": "The image preview response does not match the request.",
  "確認画像の一覧が不正です。": "The image preview list is invalid.",
  "保存の応答を確認できませんでした。": "The save response could not be verified.",
  "保存の応答が選択した画像と一致しません。": "The save response does not match the selected image.",
  "待機中": "Queued",
  "確認画像を生成中": "Generating image",
  "取消処理中": "Cancelling",
  "取消済み": "Cancelled",
  "確認画像を作成済み": "Image generated",
  "失敗": "Failed",
  "中断": "Interrupted",
  "原本固定・確認画像を生成中": "Source assets fixed; generating image",
  "確認画像の検証完了": "Image verification complete",
  "書き出しなどの処理を待っています": "Waiting for exports and other foreground work",
  "約 {seconds} 秒の画像": "Image at approximately {seconds} seconds",
  "更新 {revision} — {phase}（{id}）": "Revision {revision} — {phase} ({id})",
  "更新 {revision} ／ フレーム {frame} ／ {seconds} 秒 ／ {width} × {height}": "Revision {revision} / Frame {frame} / {seconds} seconds / {width} × {height}",
  "画像-{id}.png": "image-{id}.png"
} as const;

export type FrameMessage = keyof typeof english;
export function frameText(message: FrameMessage, values: Record<string, string | number> = {}, language = locale.current): string {
  const text = language === 'en' ? english[message] : message;
  return text.replace(/\{(\w+)\}/g, (placeholder, key: string) => key in values ? String(values[key]) : placeholder);
}
export function frameStage(stage: string): string {
  return Object.hasOwn(english, stage) ? frameText(stage as FrameMessage) : stage;
}
