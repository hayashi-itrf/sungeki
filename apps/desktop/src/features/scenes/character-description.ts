import { locale } from '../../shared/i18n/locale.svelte';
import { englishFields } from '../editor/difference-english';
import type { Character, Project } from '@sungeki/api-client';
export const voiceSettingLabels: Record<string, string> = {
  speed_scale: '話速', pitch_scale: '声の高さ', intonation_scale: '抑揚', volume_scale: '合成時の音量',
  pre_phoneme_length: '開始前の無音', post_phoneme_length: '終了後の無音', pause_length: '句読点の間',
  pause_length_scale: '句読点の間の倍率', enable_interrogative_upspeak: '疑問文の語尾上昇',
  output_sampling_rate: '音声のサンプリングレート', output_stereo: 'ステレオ出力', style_id: '話者スタイル',
  model_id: 'モデル', speaker_id: '話者', language: '言語', style: 'スタイル', style_weight: 'スタイルの強さ',
  length: '話す長さの倍率', sdp_ratio: '発話リズムの揺らぎ', noise: '声の揺らぎ', noisew: '音の長さの揺らぎ',
  model: 'モデル', voice: '話者', speed: '話速', instruct: '話し方の指示',
  voice_id: '登録話者', caption: '声の説明', cfg_scale: '条件の強さ', num_steps: '生成ステップ', seed: '乱数シード',
  auto_split: '文ごとの分割', split_interval: '分割した文の間隔',
};
function same(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (!a || !b || typeof a !== 'object' || typeof b !== 'object') return false;
  const left = Object.entries(a), right = Object.entries(b);
  return left.length === right.length && left.every(([key, value]) => Object.hasOwn(b, key) && same(value, (b as Record<string, unknown>)[key]));
}
const text = (ja: string, en: string): string => locale.current === 'en' ? en : ja;
function value(input: unknown): string {
  if (input === null) return text('エンジンに任せる', 'Engine default');
  if (typeof input === 'boolean') return input ? text('有効', 'Enabled') : text('無効', 'Disabled');
  return String(input);
}
function subtitle(style: Character['subtitle_style']): string {
  if (locale.current === 'en') return `${style.layout?.alignment ?? 'left'} aligned, wrapping ${style.layout?.wrap ? 'on' : 'off'}, ${style.font_size}px, text color ${style.color}, outline ${style.outline ? style.outline.width + 'px ' + style.outline.color : 'none'}`;
  const align = { left: '左揃え', center: '中央揃え', right: '右揃え' };
  return `${align[style.layout?.alignment ?? 'left']}・折返し${style.layout?.wrap ? 'あり' : 'なし'}・${style.font_size}px・文字色${style.color}・縁取り${style.outline ? `${style.outline.width}px ${style.outline.color}` : 'なし'}`;
}
export function describeCharacter(after: Character, project: Project): string {
  const before = project.characters.find(c => c.id === after.id);
  if (!before && locale.current === 'en') return `Register speaker “${after.name}”. Speech engine: ${{ voicevox: 'VOICEVOX', sbv2: 'SBV2', qwen: 'Qwen', irodori: 'Irodori' }[after.voice.type]}`;
  if (!before) return `話者「${after.name}」を登録。音声エンジン：${{ voicevox: 'VOICEVOX', sbv2: 'SBV2', qwen: 'Qwen', irodori: 'Irodori' }[after.voice.type]}`;
  const changes: string[] = [];
  if (before.name !== after.name) changes.push(`${text('名前：', 'Name: ')}${before.name} → ${after.name}`);
  if (before.credit !== after.credit) changes.push(text('クレジットを変更', 'Credits changed'));
  if (!same(before.subtitle_style, after.subtitle_style)) changes.push(`${text('字幕：', 'Subtitles: ')}${subtitle(before.subtitle_style)} → ${subtitle(after.subtitle_style)}`);
  if (!same(before.voice, after.voice)) {
    if (before.voice.type !== after.voice.type) changes.push(text('音声エンジンを変更', 'Speech engine changed'));
    else {
      const previous = before.voice.settings as unknown as Record<string, unknown>;
      for (const [key, next] of Object.entries(after.voice.settings)) {
        if (!same(previous[key], next)) changes.push(`${locale.current === 'en' ? (key === 'voice' ? 'Speaker' : englishFields[key] ?? 'Voice settings') : voiceSettingLabels[key] ?? '音声設定'}${text('：', ': ')}${value(previous[key])} → ${value(next)}`);
      }
      if (before.voice.type === 'voicevox' && after.voice.type === 'voicevox' && (before.voice.engine_id !== after.voice.engine_id || before.voice.core_version !== after.voice.core_version)) changes.push(text('音声エンジンの識別情報を変更', 'Speech engine identity changed'));
      if (before.voice.type !== 'voicevox' && after.voice.type !== 'voicevox' && !same(before.voice.identity, after.voice.identity)) changes.push(text('モデル原本の識別情報を変更', 'Model original identity changed'));
    }
    const speech = project.clips.filter(c => c.content.type === 'speech' && c.content.character_id === after.id);
    const generated = speech.filter(c => c.content.type === 'speech' && c.content.audio !== null).length;
    if (locale.current === 'en') changes.push(generated
      ? `Affected speech clips: ${speech.length}. Clips with generated audio: ${generated}. This audio will be detached and must be regenerated. Original assets are kept.`
      : `Affected speech clips: ${speech.length}. No generated audio is attached. Changes apply to the next synthesis.`);
    else changes.push(generated ? `対象セリフ${speech.length}件。生成済み音声${generated}件が外れ、再生成が必要です（元の素材は保持）` : `対象セリフ${speech.length}件。生成済み音声はありません。次回の合成に反映します`);
  }
  if (locale.current === 'en') return `Speaker “${after.name}”: ${changes.length ? changes.join('. ') : 'No changes'}`;
  return `話者「${after.name}」：${changes.length ? changes.join('。') : '変更なし'}`;
}
