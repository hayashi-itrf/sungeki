import type { BuildStanding, Clip, Preset, Project } from '@sungeki/api-client';

/** Capture settings only; confirmed Project changes always go through host. */
export function captureCharacter(project: Project, characterId: string, clipId: string, name: string): Preset {
  const character = project.characters.find(c => c.id === characterId);
  if (!character) throw new Error('キャラクターを選択してください / Select a character');
  const clip = clipId ? project.clips.find(c => c.id === clipId) : null;
  if (clipId && !clip) throw new Error('立ち絵が見つかりません / Standing not found');
  let standing: BuildStanding | null = null;
  const assetIds = new Set<string>();
  if (clip) {
    const c = clip.content, flip_x = clip.transform.flip_x ?? false;
    if (c.type === 'image') { standing = { ...c, flip_x }; assetIds.add(c.asset_id); }
    else if (c.type === 'psd' && c.speech?.character_id === characterId) {
      standing = { type: 'psd', asset_id: c.asset_id, pose: c.pose, speech: c.speech, flip_x }; assetIds.add(c.asset_id);
    } else if (c.type === 'sprite' && c.character_id === characterId) {
      standing = { type: 'sprite', sprites: c.sprites, pose: c.pose, flip_x };
      for (const p of c.sprites.poses) { assetIds.add(p.closed_asset_id); if (p.open_asset_id) assetIds.add(p.open_asset_id); }
    } else if (c.type === 'vrm' && c.standing.character_id === characterId) {
      standing = { type: 'vrm', standing: c.standing, flip_x };
      assetIds.add(c.standing.model_asset_id); assetIds.add(c.standing.motion_asset_id);
    } else throw new Error('人物と立ち絵の関連付けを確認してください / Check character binding');
  }
  const assets = project.assets.filter(a => assetIds.has(a.id));
  if (assets.length !== assetIds.size) throw new Error('参照素材が不足しています / Missing asset references');
  return structuredClone({ schema_version: 1, name, description: '', tags: [], content: { kind: 'character', character, assets, standing } });
}

export function isStandingFor(clip: Clip, characterId: string): boolean {
  const c = clip.content;
  return c.type === 'image' || (c.type === 'psd' && c.speech?.character_id === characterId)
    || (c.type === 'sprite' && c.character_id === characterId) || (c.type === 'vrm' && c.standing.character_id === characterId);
}

export function captureVoice(project: Project, characterId: string, name: string): Preset {
  const character = project.characters.find(c => c.id === characterId);
  if (!character) throw new Error('キャラクターを選択してください / Select a character');
  return structuredClone({ schema_version: 1, name, description: '', tags: [], content: { kind: 'voice', voice: character.voice, credit: character.credit } });
}
