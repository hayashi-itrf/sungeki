import { locale } from '../../shared/i18n/locale.svelte';
import type { BuildRecipe, BuildRole, Project } from '@sungeki/api-client';
import { scenesFromForm, type SceneInput } from './scenes';
import { forewordFromForm, type ForewordInput } from './foreword';
import { standingPreset } from './standing-preset';
import type { BuildProfile } from './voice-settings';
export type BuildFormInput = {
  preset?: 'dialogue-wide-v1' | 'narration-wide-v1' | 'narration-portrait-v1';
  title: string; scenes: SceneInput[]; bgm: string;
  foreword?: ForewordInput;
  fitBgmVolumePoints?: boolean;
  people: { character: string; standing: string; profile?: BuildProfile }[];
};
// Only translate form values to the common API's input. The host validates
// originals/presets and performs all timeline construction and Project edits.
export function recipeFromForm(project: Project, input: BuildFormInput): BuildRecipe {
  if (!input.title.trim()) throw new Error(locale.text('productionValidationTitle'));
  const preset = input.preset ?? 'dialogue-wide-v1';
  if (!['dialogue-wide-v1', 'narration-wide-v1', 'narration-portrait-v1'].includes(preset)) throw new Error(locale.text('productionValidationPreset'));
  const narration = preset !== 'dialogue-wide-v1';
  const people = narration ? input.people.slice(0, 1) : input.people;
  if (people.length !== (narration ? 1 : 2)) throw new Error(locale.text('productionValidationPeople'));
  if (!narration && people[0]!.character === people[1]!.character) throw new Error(locale.text('productionValidationDistinctPeople'));
  const foreword = forewordFromForm(input.foreword, preset === 'narration-portrait-v1');
  const scenes = scenesFromForm(project, input.scenes);
  if (narration) {
    const index = scenes.findIndex(scene => scene.lines.some(line => line.role !== 'left'));
    if (index !== -1) throw new Error(locale.text('productionValidationNarration').replace('{scene}', String(index + 1)));
  }
  const assets = new Set<string>(scenes.flatMap(scene => [scene.background_asset_id, ...scene.diagrams.map(d => d.asset_id)]));
  const roles: BuildRole[] = people.map((person, index) => {
    const character = project.characters.find(c => c.id === person.character);
    const clip = project.clips.find(c => c.id === person.standing);
    if (!character || !clip || !['image', 'psd', 'sprite', 'vrm'].includes(clip.content.type)) throw new Error(locale.text('productionValidationStanding'));
    const content = clip.content;
    if (content.type !== 'image' && content.type !== 'psd' && content.type !== 'sprite' && content.type !== 'vrm') throw new Error(locale.text('productionValidationStandingType'));
    if (content.type === 'vrm') {
      assets.add(content.standing.model_asset_id); assets.add(content.standing.motion_asset_id);
    } else if (content.type === 'sprite') {
      for (const pose of content.sprites.poses) {
        assets.add(pose.closed_asset_id); if (pose.open_asset_id) assets.add(pose.open_asset_id);
      }
    } else assets.add(content.asset_id);
    const standing: BuildRole['standing'] = content.type === 'vrm'
      ? (() => {
        if (content.standing.character_id !== character.id) throw new Error(locale.text('productionValidationVrmSpeaker'));
        return { type: 'vrm' as const, standing: content.standing, flip_x: clip.transform.flip_x ?? false };
      })()
      : content.type === 'sprite'
      ? (() => {
        if (content.character_id !== character.id) throw new Error(locale.text('productionValidationSpriteSpeaker'));
        return { type: 'sprite' as const, sprites: content.sprites, pose: content.pose, flip_x: clip.transform.flip_x ?? false };
      })()
      : content.type === 'image'
      ? { type: 'image', asset_id: content.asset_id, flip_x: clip.transform.flip_x ?? false }
      : (() => {
        if (!content.speech || content.speech.character_id !== character.id) throw new Error(locale.text('productionValidationPsdSpeaker'));
        return { type: 'psd' as const, asset_id: content.asset_id, pose: content.pose, speech: content.speech, flip_x: clip.transform.flip_x ?? false };
      })();
    const presetId = content.type === 'vrm' ? 'vrm-custom-v1' : content.type === 'sprite' ? 'sprite-custom-v1' : content.type === 'image' ? 'image-custom-v1' : standingPreset(content, character.voice.type)?.preset_id;
    if (!presetId) throw new Error(locale.text('productionValidationPsdPreset'));
    return { role: index === 0 ? 'left' : 'right', preset_id: presetId, character, standing };
  });
  let bgm: BuildRecipe['bgm'] = null;
  if (input.bgm) {
    const clip = project.clips.find(c => c.id === input.bgm);
    if (clip?.content.type !== 'audio') throw new Error(locale.text('productionValidationBgm'));
    bgm = { ...(input.fitBgmVolumePoints ? { volume_point_reference_duration_us: clip.duration_us } : {}), asset_id: clip.content.asset_id, playback: { ...clip.content.playback, looped: true } };
    assets.add(bgm.asset_id);
  }
  const selected = [...assets].map(id => {
    const asset = project.assets.find(a => a.id === id);
    if (!asset || asset.locator.type !== 'absolute' || !asset.credit.trim()) throw new Error(locale.text('productionValidationCredits'));
    return asset;
  });
  return JSON.parse(JSON.stringify({ preset_id: preset, title: input.title, assets: selected, roles,
    scenes,
    bgm, foreword, audio_mix: project.audio_mix, pronunciation_dictionary: project.pronunciation_dictionary })) as BuildRecipe;
}
