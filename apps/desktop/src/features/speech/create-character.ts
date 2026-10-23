import { extraText } from '../../shared/i18n/extra-ui';
import type { Character, VoicevoxSpeakerDetails } from '@sungeki/api-client';
import { newCharacterSubtitles } from './new-character-subtitles';
export function createCharacter(details: VoicevoxSpeakerDetails, styleId: number, name: string, credit: string, background = '#ffffff'): Character {
  if (!details.speaker.styles.some(style => style.id === styleId && ['talk', 'streaming_talk'].includes(style.kind))) throw new Error(extraText("利用できる声を選び直してください。"));
  if (!name.trim()) throw new Error(extraText("キャラクター名を入力してください。"));
  if (!Number.isSafeInteger(details.default_sampling_rate) || details.default_sampling_rate <= 0) throw new Error(extraText("音声の出力形式を再取得してください。"));
  return {
    id: crypto.randomUUID(), name, credit, subtitle_style: newCharacterSubtitles(background),
    voice: { type: 'voicevox', engine_id: details.engine.engine_id, core_version: details.core_version, settings: {
      style_id: styleId, speed_scale: 1, pitch_scale: 0, intonation_scale: 1, volume_scale: 1,
      pre_phoneme_length: .1, post_phoneme_length: .1, pause_length: null, pause_length_scale: 1,
      output_sampling_rate: details.default_sampling_rate, output_stereo: false, enable_interrogative_upspeak: true,
    } },
  };
}
