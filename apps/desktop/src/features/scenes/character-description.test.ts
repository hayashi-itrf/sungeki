import { afterEach, expect, test } from 'vitest';
import { locale } from '../../shared/i18n/locale.svelte';
afterEach(() => { locale.current = 'ja'; });
import type { Character, Project, ProjectDifference } from '@sungeki/api-client';
import { describeCommand } from './command-description';
import { differenceLabel } from '../editor/difference-labels';
const character: Character = { id: 'voice', name: '語り手', credit: 'credit', subtitle_style: { font_size: 64, color: '#000000', layout: null, outline: null },
 voice: { type: 'voicevox', engine_id: 'engine', core_version: 'core', settings: { style_id: 1, speed_scale: 1, pitch_scale: 0, intonation_scale: 1, volume_scale: 1, pre_phoneme_length: .1, post_phoneme_length: .1, pause_length: null, pause_length_scale: 1, output_sampling_rate: 24000, output_stereo: false, enable_interrogative_upspeak: true } } };
function project(): Project {
 return { characters: [structuredClone(character)], clips: [
  { id: 'generated', content: { type: 'speech', character_id: 'voice', audio: { asset_id: 'original' } } },
  { id: 'pending', content: { type: 'speech', character_id: 'voice', audio: null } },
  { id: 'other', content: { type: 'speech', character_id: 'other', audio: { asset_id: 'other-original' } } },
  { id: 'bgm', content: { type: 'audio', asset_id: 'bgm' } },
 ] } as Project;
}
test('voice review shows changed values and only the affected speaker audio count', () => {
 const before = project(), after = structuredClone(character);
 if(after.voice.type !== 'voicevox') throw new Error('fixture');
 after.voice.settings.speed_scale = 1.2; after.voice.settings.pause_length = 0;
 const text = describeCommand({ type: 'set_character', character: after }, before);
 expect(text).toContain('話速：1 → 1.2');
 expect(text).toContain('句読点の間：エンジンに任せる → 0');
 expect(text).toContain('対象セリフ2件'); expect(text).toContain('生成済み音声1件');
 expect(text).not.toContain('字幕：');
 expect(before.characters[0]).toEqual(character);
});
test('subtitle-only changes and identical reordered settings never warn about audio regeneration', () => {
 const before = project(), after = structuredClone(character);
 after.subtitle_style.font_size = 72;
 const text = describeCommand({ type: 'set_character', character: after }, before);
 expect(text).toContain('64px'); expect(text).toContain('72px'); expect(text).not.toContain('再生成');
 const same = structuredClone(character);
 same.voice.settings = Object.fromEntries(Object.entries(same.voice.settings).reverse()) as typeof same.voice.settings;
 expect(describeCommand({ type: 'set_character', character: same }, before)).toContain('変更なし');
});
test('new characters and engine/model changes have distinct explanations', () => {
 const before = project(), next = structuredClone(character);
 next.id = 'new'; expect(describeCommand({ type: 'set_character', character: next }, before)).toContain('登録');
 next.id = character.id;
 next.voice = { type: 'sbv2', identity: { engine_revision: 'version', model_sha256: 'a', config_sha256: 'b', style_vectors_sha256: 'c', text_encoder_fingerprint: 'd' }, settings: { model_id: 0, speaker_id: 0, language: 'JP', style: 'Neutral', style_weight: 1, length: 1, sdp_ratio: .2, noise: .6, noisew: .8, auto_split: true, split_interval: .5 } };
 expect(describeCommand({ type: 'set_character', character: next }, before)).toContain('音声エンジンを変更');
 before.characters = [structuredClone(next)]; next.voice.identity.model_sha256 = 'changed';
 expect(describeCommand({ type: 'set_character', character: next }, before)).toContain('モデル原本の識別情報');
 before.clips = []; expect(describeCommand({ type: 'set_character', character: next }, before)).toContain('生成済み音声はありません');
});
test('structured diff uses understandable labels for voice settings', () => {
 const entry = { collection: 'characters', field: '/voice/settings/speed_scale', kind: 'changed' } as ProjectDifference;
 expect(differenceLabel(entry)).toBe('キャラクターの音声・設定・話速：変更');
 expect(differenceLabel({ ...entry, field: '/voice/settings/style_weight' })).toContain('スタイルの強さ');
});
test('Qwen speaker setting is distinct from the voice object label', () => {
 const entry = { collection: 'characters', field: '/voice/settings/voice', kind: 'changed' } as ProjectDifference;
 expect(differenceLabel(entry)).toBe('キャラクターの音声・設定・話者：変更');
});

test('English review explains regeneration for the affected speaker and preserves project content', () => {
 locale.current = 'en';
 const before = project(), original = JSON.stringify(before), after = structuredClone(character);
 if (after.voice.type !== 'voicevox') throw new Error('fixture');
 after.voice.settings.speed_scale = 1.2;
 after.voice.settings.pause_length = 0;
 const text = describeCommand({ type: 'set_character', character: after }, before);
 expect(text).toContain('Speaker “語り手”');
 expect(text).toContain('Speech speed: 1 → 1.2');
 expect(text).toContain('Punctuation pause: Engine default → 0');
 expect(text).toContain('Affected speech clips: 2');
 expect(text).toContain('Clips with generated audio: 1. This audio will be detached and must be regenerated');
 expect(text).toContain('Original assets are kept');
 expect(JSON.stringify(before)).toBe(original);
});

test('English subtitle edits do not imply regeneration and unchanged settings are recognized', () => {
 locale.current = 'en';
 const before = project(), after = structuredClone(character);
 after.subtitle_style.font_size = 72;
 const text = describeCommand({ type: 'set_character', character: after }, before);
 expect(text).toContain('Subtitles:');
 expect(text).toContain('64px'); expect(text).toContain('72px');
 expect(text).not.toContain('regenerated');
 expect(describeCommand({ type: 'set_character', character }, before)).toContain('No changes');
});
