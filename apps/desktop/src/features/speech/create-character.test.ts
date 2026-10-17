import { describe, expect, it } from 'vitest';
import type { VoicevoxSpeakerDetails } from '@sungeki/api-client';
import { createCharacter } from './create-character';
const details: VoicevoxSpeakerDetails = {
  engine: { engine_id: 'engine', name: 'VOICEVOX', version: '1', core_versions: ['0.16.0'] },
  core_version: '0.16.0', speaker: { speaker_uuid: 'speaker', name: '話者', version: '1', styles: [{ id: 42, name: '通常', kind: 'talk' }] },
  engine_terms: 'engine terms', speaker_policy: 'speaker policy', default_sampling_rate: 48000,
};
describe('character registration', () => {
  it('keeps subtitles visible on light and dark solid backgrounds', () => {
    expect(createCharacter(details, 42, 'name', '', '#ffffff').subtitle_style.color).toBe('#000000');
    expect(createCharacter(details, 42, 'name', '', '#000000').subtitle_style.color).toBe('#ffffff');
    expect(createCharacter(details, 42, 'name', '', '#FFFF00').subtitle_style.color).toBe('#000000');
    expect(createCharacter(details, 42, 'name', '', '#0000ff').subtitle_style.color).toBe('#ffffff');
  });
  it('uses the selected engine/core/style and preserves user credit without changing metadata', () => {
    const before = structuredClone(details);
    const character = createCharacter(details, 42, 'ナレーター', '作者名\n利用条件の記載');
    expect(character.subtitle_style).toEqual({ font_size: 64, color: '#000000', layout: { alignment: 'left', wrap: true }, outline: { width: 3, color: '#ffffff' } });
    expect(createCharacter(details, 42, '暗背景', '', '#000000').subtitle_style).toEqual({ font_size: 64, color: '#ffffff', layout: { alignment: 'left', wrap: true }, outline: { width: 3, color: '#000000' } });
    expect(character.voice.type).toBe('voicevox');
    if (character.voice.type !== 'voicevox') throw new Error('VOICEVOX fixture required');
    expect(character.voice.engine_id).toBe('engine');
    expect(character.voice.core_version).toBe('0.16.0');
    expect(character.voice.settings.style_id).toBe(42);
    expect(character.voice.settings.output_sampling_rate).toBe(48000);
    expect(character.credit).toBe('作者名\n利用条件の記載');
    expect(details).toEqual(before);
    expect(createCharacter(details, 42, '次の話者', '').id).not.toBe(character.id);
  });
  it('rejects foreign/singing styles, empty names and unavailable output format', () => {
    expect(() => createCharacter(details, 1, 'name', '')).toThrow();
    expect(() => createCharacter(details, 42, ' ', '')).toThrow();
    expect(() => createCharacter({ ...details, default_sampling_rate: 0 }, 42, 'name', '')).toThrow();
    expect(() => createCharacter({ ...details, speaker: { ...details.speaker, styles: [{ id: 42, name: '歌', kind: 'sing' }] } }, 42, 'name', '')).toThrow();
  });
});
