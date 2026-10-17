export const previewStatusEnglish = {
  "previewSourceVideo": "Source video frame",
  "previewProxyVideo": "Proxy video frame",
  "previewAssetImage": "Asset image",
  "previewImageLoadingTitle": "Loading image",
  "previewImageLoading": "Loading image…",
  "previewImageUnavailable": "Cannot display image",
  "previewProxyUnavailable": "Cannot display proxy",
  "previewVrmHint": "VRM: check the rendered frame",
  "waveformUnexpected": "Could not retrieve the waveform.",
  "waveformLoadingTitle": "Loading the source audio waveform",
  "waveformUnavailable": "Waveform unavailable (reload to retry)",
  "waveformLoading": "Loading waveform…"
} as const;
export const previewStatusJapanese: Record<keyof typeof previewStatusEnglish, string> = {
  "previewSourceVideo": "原本の動画画像",
  "previewProxyVideo": "軽量版の動画画像",
  "previewAssetImage": "素材画像",
  "previewImageLoadingTitle": "画像を読み込んでいます",
  "previewImageLoading": "画像を読込中",
  "previewImageUnavailable": "画像を表示できません",
  "previewProxyUnavailable": "軽量版を表示できません",
  "previewVrmHint": "VRM：正式な画像で確認",
  "waveformUnexpected": "波形を取得できませんでした。",
  "waveformLoadingTitle": "元音声の波形を取得中",
  "waveformUnavailable": "波形未取得（再読込で確認）",
  "waveformLoading": "波形を取得中…"
};
