import { tuningText as t } from './tuning-messages';
import type { Character, Command, IrodoriSettings } from '@sungeki/api-client';
export type IrodoriTuning = Pick<IrodoriSettings, 'caption' | 'cfg_scale' | 'num_steps' | 'seed'>;
export function tuneIrodori(character: Character, tuning: IrodoriTuning): Command {
  if (character.voice.type !== 'irodori') throw new Error(t("Irodoriのキャラクターを選んでください。"));
  if (!Number.isFinite(tuning.cfg_scale) || tuning.cfg_scale < 0 || tuning.cfg_scale > 30)
    throw new Error(t("条件への追従度は0〜30で指定してください。"));
  if (!Number.isInteger(tuning.num_steps) || tuning.num_steps < 1 || tuning.num_steps > 200)
    throw new Error(t("生成ステップ数は1〜200の整数で指定してください。"));
  if (!Number.isInteger(tuning.seed) || tuning.seed < 0 || tuning.seed > 4294967295)
    throw new Error(t("生成シードは0〜4294967295の整数で指定してください。"));
  if (tuning.caption !== null && (typeof tuning.caption !== 'string' || !tuning.caption.trim()
    || tuning.caption.includes('\0') || new TextEncoder().encode(tuning.caption).length > 4096))
    throw new Error(t("声の説明は空欄、または4096バイト以内の文章で指定してください。"));
  const settings = { ...character.voice.settings, caption: tuning.caption, cfg_scale: tuning.cfg_scale,
    num_steps: tuning.num_steps, seed: tuning.seed };
  return { type: 'set_character', character: structuredClone({ ...character, voice: { ...character.voice, settings } }) };
}
