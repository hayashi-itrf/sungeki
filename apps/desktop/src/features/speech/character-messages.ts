import { locale } from '../../shared/i18n/locale.svelte';

const english = {
  "＋ キャラクター": "＋ Character",
  "キャラクターを登録": "Register character",
  "入力を取り消して閉じる": "Discard input and close",
  "閉じる": "Close",
  "音声エンジン": "Voice engine",
  "作品内のキャラクター名": "Character name in this project",
  "例：ナレーター": "Example: Narrator",
  "クレジット": "Credits",
  "利用条件を確認して記入してください": "Check the usage terms before entering credits.",
  "標準の話速・音量で登録します。字幕は現在の背景色に合わせて黒か白を選びます。背景画像や後からの背景変更には自動追従しません。登録だけでは音声は生成されません。": "Registers with default speed and volume. Subtitle text uses black or white based on the current background color. It does not automatically adapt to background images or later background changes. Registration does not generate audio.",
  "声の情報を再確認中…": "Rechecking voice details…",
  "声の情報または利用条件が変わりました。一覧から選び直してください。": "Voice details or usage terms have changed. Select the voice again from the list.",
  "起動済みのVOICEVOXに接続して声を選びます。登録後にセリフを追加し、「音声合成」から生成できます。エンジンは先に起動してください。": "Connect to a running VOICEVOX engine to select a voice. After registration, add dialogue and use “Speech synthesis” to generate audio. Start the engine first.",
  "エンジンの接続先": "Engine connection",
  "接続先": "Address",
  "声の一覧を取得": "Load voices",
  "音声エンジンのバージョン": "Voice engine version",
  "選択してください": "Select",
  "話者": "Speaker",
  "話者の詳細・利用条件を取得": "Load speaker details and usage terms",
  "スタイル": "Style",
  "エンジンの利用条件": "Engine usage terms",
  "話者の利用条件": "Speaker usage terms",
  "利用条件が提供されていません。配布元で確認してください。": "No usage terms were provided. Check with the distributor.",
  "声の情報を取得しています…": "Loading voice details…",
  "接続情報の応答が不正です。": "The engine connection response is invalid.",
  "声一覧の応答が不正です。": "The voice list response is invalid.",
  "このエンジンには利用できる読み上げ話者がありません。": "This engine has no available text-to-speech speakers.",
  "話者情報の応答が不正です。": "The speaker details response is invalid."
} as const;

export function characterText(message: keyof typeof english): string {
  return locale.current === 'en' ? english[message] : message;
}

export function characterDiagnostic(message: string): string {
  return locale.current === 'en' && Object.hasOwn(english, message)
    ? english[message as keyof typeof english] : message;
}
