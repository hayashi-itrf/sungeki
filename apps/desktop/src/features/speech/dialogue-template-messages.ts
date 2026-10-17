import { locale } from '../../shared/i18n/locale.svelte';

const english = {
  "掛け合いの構成を整える": "Arrange a two-person dialogue",
  "掛け合いの構成": "Two-person dialogue layout",
  "閉じる": "Close",
  "横動画（1920×1080・30fps）用です。先に二人の音声を生成し、背景画像・立ち絵2枚・BGMを追加してください。素材と話者のクレジットも必要です。": "For landscape video (1920×1080 at 30 fps). Generate speech for two characters first, then add a background image, two character images and BGM. Asset and speaker credits are required.",
  "タイトル3秒 → 掛け合い → クレジット3.5秒を作ります。背景・立ち絵・字幕の配置を変更し、BGMは全体で繰り返し、−18dB・前後1秒フェードに設定します。立ち絵のポーズと口形設定は維持します。映像フェードはまだ付きません。": "Creates a 3-second title, the dialogue, and 3.5 seconds of credits. Repositions the background, characters and subtitles. BGM loops throughout at −18 dB with 1-second fades at both ends. Character poses and mouth settings are preserved. Video fades are not added.",
  "シーン未所属のクリップを選んでください。対象外のクリップと重なる時間には配置できません。全変更は一度の「元に戻す」で取り消せます。": "Select clips that do not belong to a scene. The layout cannot overlap unselected clips in time. One Undo reverses all changes.",
  "作品が更新されました。最新の状態を読み込んでください。": "The project has changed. Load the latest state.",
  "入力を破棄して最新の状態を読む": "Discard input and load the latest state",
  "タイトル": "Title",
  "開始位置（秒）": "Start (seconds)",
  "発話間隔（秒）": "Gap between lines (seconds)",
  "左の話者": "Left speaker",
  "左の立ち絵": "Left character image",
  "右の話者": "Right speaker",
  "右の立ち絵": "Right character image",
  "選択": "Select",
  "背景": "Background",
  "セリフ（開始時刻順）": "Dialogue (in start-time order)",
  "変更内容を確認": "Review changes"
} as const;

export function dialogueText(message: keyof typeof english): string {
  return locale.current === 'en' ? english[message] : message;
}
