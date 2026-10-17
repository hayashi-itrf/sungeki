import { matchingAudio } from './reuse.svelte';
import { locale } from '../../shared/i18n/locale.svelte';
import type { BuildRecipe, BuildReuseRequest, BuildReuseSource, HostApi, Voice } from '@sungeki/api-client';
export type ManagedVoice = Exclude<Voice['type'], 'voicevox'>;
export type BuildProfile = { engine: ManagedVoice; id: string };
export function buildProfiles(recipe: BuildRecipe, people: { character: string; profile?: BuildProfile }[]) {
  const profiles: Record<string, string> = {};
  for (const role of recipe.roles) {
    if (role.character.voice.type === 'voicevox') continue;
    const selection = people.find(p => p.character === role.character.id)?.profile;
    if (!selection || selection.engine !== role.character.voice.type || !/^[A-Za-z0-9_-]{1,64}$/.test(selection.id)) {
      throw new Error(locale.text('productionValidationVoiceProfile').replace('{name}', () => role.character.name));
    }
    profiles[role.character.id] = selection.id;
  }
  return profiles;
}
export async function buildConnection(api: Pick<HostApi, 'session'>, recipe: BuildRecipe, url: string, reuse?: BuildReuseRequest, source?: BuildReuseSource | null) {
  const voices = recipe.roles.flatMap(r => r.character.voice.type === 'voicevox' ? [r.character.voice] : []);
  if (!voices.length) return { base_url: '', engine_version: '' };
  const saved = savedVoicevoxConnection(recipe, reuse, source);
  if (saved) return saved;
  const response = await api.session({ type: 'discover_voicevox', args: { base_url: url } });
  if (response.type !== 'voicevox_engine') throw new Error(locale.text('productionValidationVoicevoxUnavailable'));
  if (voices.some(v => v.engine_id !== response.engine.engine_id || !response.engine.core_versions.includes(v.core_version))) {
    throw new Error(locale.text('productionValidationVoicevoxMismatch'));
  }
  return { base_url: url, engine_version: response.engine.version };
}

export function savedVoicevoxConnection(recipe: BuildRecipe, reuse?: BuildReuseRequest, source?: BuildReuseSource | null) {
  const lines = recipe.scenes.flatMap(scene => scene.lines);
  // This only skips discovery. Host Start still verifies the fixed source and each original.
  if (reuse && source && reuse.parent_job_id === source.job_id && source.base_url && source.engine_version
    && lines.length > 0 && Object.keys(reuse.speech).length === lines.length
    && lines.every((_, index) => matchingAudio(recipe, index, source).some(line => line.index === reuse.speech[index]))) {
    return { base_url: source.base_url, engine_version: source.engine_version };
  }
  return null;
}
