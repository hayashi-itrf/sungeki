import { tuningText as t } from './tuning-messages';
import type { Character, Command, VoicevoxSettings } from '@sungeki/api-client';
export const voicevoxTuningFields = [
  { key: 'speed_scale', label: '話速（1が標準、大きいほど速い）', min: 0 },
  { key: 'pitch_scale', label: '声の高さ（0が標準）', min: undefined },
  { key: 'intonation_scale', label: '抑揚（1が標準）', min: undefined },
  { key: 'volume_scale', label: '合成時の音量（1が標準）', min: 0 },
  { key: 'pre_phoneme_length', label: '開始前の無音（秒）', min: 0 },
  { key: 'post_phoneme_length', label: '終了後の無音（秒）', min: 0 },
  { key: 'pause_length_scale', label: '句読点の間の倍率（1が標準）', min: 0 },
] as const;
export type VoicevoxTuning = Pick<VoicevoxSettings, typeof voicevoxTuningFields[number]['key'] | 'pause_length' | 'enable_interrogative_upspeak'>;
export function tuneVoicevox(character: Character, tuning: VoicevoxTuning): Command {
  if (character.voice.type !== 'voicevox') throw new Error(t("VOICEVOXのキャラクターを選んでください。"));
  const settings = { ...character.voice.settings };
  for (const field of voicevoxTuningFields) {
    const value = tuning[field.key];
    if (!Number.isFinite(value) || (field.min !== undefined && value < field.min) || (field.key === 'speed_scale' && value === 0)) throw new Error(t("{field}の値を確認してください。", { field: t(field.label) }));
    settings[field.key] = value;
  }
  if (tuning.pause_length !== null && (!Number.isFinite(tuning.pause_length) || tuning.pause_length < 0)) throw new Error(t("句読点の間は0以上の秒数で指定してください。"));
  if (typeof tuning.enable_interrogative_upspeak !== 'boolean') throw new Error(t("疑問文の語尾設定を確認してください。"));
  settings.pause_length = tuning.pause_length;
  settings.enable_interrogative_upspeak = tuning.enable_interrogative_upspeak;
  return { type: 'set_character', character: structuredClone({ ...character, voice: { ...character.voice, settings } }) };
}
