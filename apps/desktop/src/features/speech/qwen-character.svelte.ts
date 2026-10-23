import { extraText } from '../../shared/i18n/extra-ui';
import type { Character, HostApi, QwenProfileDetails } from '@sungeki/api-client';
import { newCharacterSubtitles } from './new-character-subtitles';

export function qwenCharacter(details: QwenProfileDetails, speaker: string, language: string, name: string, credit: string, background: string): Character {
  if (!name.trim()) throw new Error(extraText("キャラクター名を入力してください。"));
  if (!details.speakers.some(s => s.id === speaker) || !details.languages.includes(language)) throw new Error(extraText("話者・言語を選び直してください。"));
  return { id: crypto.randomUUID(), name, credit,
    subtitle_style: newCharacterSubtitles(background),
    voice: { type: 'qwen', identity: structuredClone(details.identity), settings: {
      ...structuredClone(details.default_settings), voice: speaker, language,
    } },
  };
}
export class QwenCharacterDraft {
  details = $state.raw<QwenProfileDetails | null>(null);
  speaker = $state('');
  language = $state('Japanese');
  busy = $state(false);
  error = $state('');
  private generation = 0;
  private profile = '';
  private alive = true;
  constructor(private api: Pick<HostApi, 'session'>) {}
  select(id: string) {
    if (id === this.profile) return;
    this.profile = id; this.generation++; this.details = null; this.busy = false; this.error = '';
  }
  dispose() { this.alive = false; this.generation++; }
  private async fetch(id: string) {
    const response = await this.api.session({ type: 'speech_job', request: { type: 'get_qwen_profile', args: { profile_id: id } } });
    if (response.type !== 'qwen_profile' || response.details.profile_id !== id) throw new Error(extraText("声の確認結果が選択した起動設定と一致しません。"));
    return structuredClone(response.details);
  }
  async inspect() {
    if (!this.profile || this.busy || !this.alive) return;
    const generation = ++this.generation, id = this.profile;
    this.busy = true; this.error = ''; this.details = null;
    try {
      const details = await this.fetch(id);
      if (!this.alive || generation !== this.generation) return;
      const initial = details.default_settings;
      if (!details.speakers.some(s => s.id === initial.voice) || !details.languages.includes(initial.language)) throw new Error(extraText("声の初期設定を確認できませんでした。"));
      this.details = details; this.speaker = initial.voice; this.language = initial.language;
    } catch (error) { if (this.alive && generation === this.generation) this.error = message(error); }
    finally { if (this.alive && generation === this.generation) this.busy = false; }
  }
  async character(name: string, credit: string, background: string): Promise<Character | null> {
    if (!this.details || this.busy || !this.alive) return null;
    const generation = ++this.generation, captured = this.details;
    this.busy = true; this.error = '';
    try {
      const character = qwenCharacter(captured, this.speaker, this.language, name, credit, background);
      const current = await this.fetch(captured.profile_id);
      if (!this.alive || generation !== this.generation) return null;
      if (JSON.stringify(current) !== JSON.stringify(captured)) {
        this.details = null;
        throw new Error(extraText("モデル原本または声の情報が変わりました。話者と言語を確認し直してください。"));
      }
      return character;
    } catch (error) { if (this.alive && generation === this.generation) this.error = message(error); return null; }
    finally { if (this.alive && generation === this.generation) this.busy = false; }
  }
}
function message(error: unknown) {
  return error && typeof error === 'object' && 'message' in error ? String(error.message) : String(error);
}
