import { tuningText as t } from './tuning-messages';
import type { Character, Command, Sbv2Settings } from '@sungeki/api-client';
export const tuningFields = [
  { key: 'length', label: '話す長さの倍率（大きいほどゆっくり）', min: .1, max: 10 },
  { key: 'style_weight', label: 'スタイルの強さ', min: 0, max: 100 },
  { key: 'split_interval', label: '分割した文の間隔（秒）', min: 0, max: 60 },
  { key: 'sdp_ratio', label: '発話リズムの揺らぎ（SDP）', min: 0, max: 1 },
  { key: 'noise', label: '声の揺らぎ（noise）', min: 0, max: 10 },
  { key: 'noisew', label: '音の長さの揺らぎ（noise_w）', min: 0, max: 10 },
] as const;
export type Sbv2Tuning = Pick<Sbv2Settings, typeof tuningFields[number]['key'] | 'auto_split'>;
export function tuneSbv2(character: Character, tuning: Sbv2Tuning): Command {
  if (character.voice.type !== 'sbv2') throw new Error(t("SBV2のキャラクターを選んでください。"));
  const settings = { ...character.voice.settings };
  for (const field of tuningFields) {
    const value = tuning[field.key];
    if (!Number.isFinite(value) || value < field.min || value > field.max) throw new Error(t("{field}は{min}〜{max}で指定してください。", { field: t(field.label), min: field.min, max: field.max }));
    settings[field.key] = value;
  }
  if (typeof tuning.auto_split !== 'boolean') throw new Error(t("文ごとの分割を指定してください。"));
  settings.auto_split = tuning.auto_split;
  return { type: 'set_character', character: structuredClone({ ...character, voice: { ...character.voice, settings } }) };
}
