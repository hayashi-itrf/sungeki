import { locale } from '../../shared/i18n/locale.svelte';

// Translate known display text only. Never derive check status or edit report data.
const descriptions: Record<string, string> = {
  '素材やクリップの参照、所属、時刻、設定値の確認。問題がある場合は最初の1件を報告します。': 'Checks asset and clip references, membership, timing and settings. If there are problems, only the first is reported.',
  '元ファイルの存在と内容は未検査です。登録情報だけでは原本を確認したことにはなりません。': 'Source file existence and contents have not been inspected. Registered metadata alone does not verify the originals.',
  '動画に書き出す際の文字組みによる、字幕のはみ出しや画面端との余白は未検査です。': 'Subtitle overflow and margins have not been inspected using the final video text layout.',
  '音声の重なり・切断・音量は未検査です。': 'Audio overlap, truncation and volume have not been inspected.',
  '動画から抜き出した確認用画像とその一覧は未生成です。全編の映像は確認していません。': 'Representative images and contact sheets have not been generated. The full video has not been reviewed.',
  '人間または画像を読めるAIによる映像内容の確認は未実施です。': 'The video content has not been reviewed by a person or an AI that can inspect images.',
  '登録された画像の内容と読み取りを確認しました。検査後の原本変更や映像の見た目は別途確認が必要です。': 'Registered image contents and readability were checked. Later source changes and visual appearance require separate review.',
  '画像の内容を検査しました。音声の検査範囲は音声項目を、PSD・動画は追加説明と指摘欄を参照してください。': 'Image contents were inspected. See the audio check for audio coverage, and the additional descriptions and issues for PSD and video coverage.',
  '画像の欠落・変更、または検査用原本の確保に問題があります。指摘された素材を確認してください。': 'Images are missing or changed, or source copies could not be secured for inspection. Check the assets listed in the issues.',
  '登録された画像はありません（検査対象なし）。': 'No registered images (nothing to inspect).',
  '登録済み音声の尺と発話配置を確認。原本の再検査・音量測定・聴き取りは未実施です。重なりは各発話につき最も長く続く先行発話との1件を表示します。': 'Registered audio durations and speech placement were checked. Source reinspection, volume measurement and listening have not been performed. Each speech reports one overlap with the preceding speech that ends latest.',
  '作品の構造に問題があるため発話配置は未検査です。原本・音量・聴き取りも未検査です。': 'Speech placement was not inspected because the project structure is invalid. Sources, volume and listening are also uninspected.',
  '登録尺による発話配置と、容量上限内の音声原本hash/容量・WAV形式を確認。原本の検査省略は指摘欄へ記載。圧縮音声の復号情報・音量・聴き取りは未検査です。重なりは各発話につき最長の先行発話との1件を表示します。検査後の原本変更は再確認が必要です。': 'Checked speech placement using registered durations, plus source audio hashes, sizes and WAV format within the size limit. Skipped source checks are listed as issues. Decoded compressed audio, volume and listening remain uninspected. Each speech reports one overlap with the preceding speech that ends latest. Later source changes require reinspection.',
  '代表画像だけでは全字幕・安全余白は未検査です。': 'Representative images alone do not inspect all subtitles or safe margins.',
  '文字の全区間検査を実施できませんでした。安全余白・重なり・読みやすさは未検査です。': 'Text inspection across all intervals could not be performed. Safe margins, overlap and readability remain uninspected.',
  '人間または画像を読めるAIによる内容確認は未実施です。画像生成の成功は映像内容の合格を意味しません。': 'Content has not been reviewed by a person or an AI that can inspect images. Successful image generation does not mean the video content has passed review.',
};
const additions: Record<string, string> = {
  'PSDは原本hash/容量の照合のみです。レイヤーの再解析・全ポーズの描画は未検査です。検査省略や問題は指摘欄に表示します。': 'PSD checks only compare source hashes and sizes. Layer reanalysis and rendering of all poses remain uninspected. Skipped checks and problems are listed as issues.',
  '動画は原本hash/容量の照合のみです。映像・内蔵音声の復号、時刻情報、再生品質は未検査です。検査省略や問題は指摘欄に表示します。': 'Video checks only compare source hashes and sizes. Video and embedded audio decoding, timing metadata and playback quality remain uninspected. Skipped checks and problems are listed as issues.',
  'VRM/VRMAは原本hash/容量の照合のみです。モデル・モーションの再解析と描画は未検査です。検査省略や問題は指摘欄に表示します。': 'VRM/VRMA checks only compare source hashes and sizes. Model and motion reanalysis and rendering remain uninspected. Skipped checks and problems are listed as issues.',
  'VRM/VRMA原本と描画は未検査です。': 'VRM/VRMA sources and rendering remain uninspected.',
};
export function checkDescription(text: string): string {
  if (locale.current !== 'en') return text;
  // Backend descriptions append separately scoped asset checks with a space.
  const parts = text.split(' ');
  if (Object.hasOwn(descriptions, parts[0]) && parts.slice(1).every(part => Object.hasOwn(additions, part))) {
    return [descriptions[parts[0]], ...parts.slice(1).map(part => additions[part])].join(' ');
  }
  const frames = /^代表画像(\d+)枚中(\d+)枚を生成。候補の省略(\d+)枚。画像を抜き出していない区間(\d+)か所。画像一覧(\d+)ページ中(\d+)ページ生成。全編の検査ではありません。$/.exec(text);
  if (frames) return `Generated ${frames[2]} of ${frames[1]} representative images. Omitted candidates: ${frames[3]}. Unsampled intervals: ${frames[4]}. Generated ${frames[6]} of ${frames[5]} contact sheet pages. This is not an inspection of the full video.`;
  const layout = /^文字の全(\d+)描画区間中(\d+)区間を正式フォント・描画処理で検査し、(\d+)区間ではみ出しを検出。(安全余白は未設定・未検査です。|上下左右(\d+)%の安全余白に(\d+)区間の文字が入っています。画面\/配置枠のはみ出し区間は安全余白の追加検査を省略しています。) 重なり・読みやすさは未検査です。$/.exec(text);
  if (layout) {
    const margin = layout[5] === undefined ? 'Safe margins are not configured and remain uninspected.' : `Text in ${layout[6]} intervals enters the ${layout[5]}% safe margin on each edge. Additional margin checks were skipped for intervals overflowing the canvas or layout box.`;
    return `Inspected ${layout[2]} of ${layout[1]} text rendering intervals using final fonts and rendering; overflow found in ${layout[3]} intervals. ${margin} Overlap and readability remain uninspected.`;
  }
  return text;
}

const issues: Record<string, [string, string]> = {
  ASSET_MISSING: ['画像ファイルが見つかりません', 'Image file not found.'],
  ASSET_CHANGED: ['画像が登録時から変更されています。素材を再指定してください', 'The image has changed since registration. Relink the asset.'],
  ASSET_METADATA_MISMATCH: ['画像の実際の情報が登録内容と一致しません。素材を再指定してください', 'The actual image metadata does not match its registered metadata. Relink the asset.'],
  SPEECH_AUDIO_NOT_GENERATED: ['音声が未合成のため、実際の発話尺と重なりを確認できません。', 'Speech audio has not been synthesized, so actual duration and overlap cannot be checked.'],
};
export function issueDescription(issue: { code: string; message: string }): string {
  const entry = Object.hasOwn(issues, issue.code) ? issues[issue.code] : undefined;
  return locale.current === 'en' && entry?.[0] === issue.message ? entry[1] : issue.message;
}

const freshnessMessages: Record<string, string> = {
  '登録された画像・音声・PSD・動画はありません': 'No images, audio, PSD or video assets are registered.',
  '検査後に画像・音声・PSD・動画の内容が変わっています': 'Image, audio, PSD or video contents have changed since inspection.',
  '検査時にあった画像・音声・PSD・動画が見つかりません': 'An image, audio, PSD or video asset present during inspection is now missing.',
};
export function freshnessDescription(text: string): string {
  return locale.current === 'en' && Object.hasOwn(freshnessMessages, text) ? freshnessMessages[text] : text;
}
