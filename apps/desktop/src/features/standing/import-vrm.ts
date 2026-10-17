import { locale } from '../../shared/i18n/locale.svelte';
import type { Command, Project, VrmAssetProbe } from '@sungeki/api-client';
export type VrmImportSource = { path: string; credit: string; probe: VrmAssetProbe };
export function importVrm(project: Project, model: VrmImportSource, motion: VrmImportSource,
  start: number, duration: number, sceneId: string, characterId: string,
  ids: { model: string; motion: string; track: string; clip: string }): Command[] {
  if (![start, duration].every(Number.isFinite) || start < 0 || duration <= 0
    || start + duration > 604800 || Math.round(duration * 1e6) === 0) throw Error(locale.text('standingTimeError'));
  if (model.probe.inspection.kind !== 'model' || motion.probe.inspection.kind !== 'motion') throw Error(locale.text('standingKindError'));
  if (characterId && !model.probe.inspection.expressions.includes('aa')) throw Error(locale.text('standingLipSyncError'));
  if (characterId && !project.characters.some(c => c.id === characterId)) throw Error(locale.text('standingCharacterError'));
  const register = (input: VrmImportSource, id: string): Command => {
    if (!input.path.startsWith('/') || !input.credit.trim()) throw Error(locale.text('standingCreditError'));
    return { type: 'register_asset', asset: { id, name: input.path.split('/').pop()!, source: input.path,
      locator: { type: 'absolute', path: input.path }, content_hash: input.probe.sha256,
      credit: input.credit.trim(), probe: { type: 'vrm', info: structuredClone(input.probe) } } };
  };
  const width = Math.min(650, project.canvas.width), height = Math.min(900, project.canvas.height);
  return [register(model, ids.model), register(motion, ids.motion),
    { type: 'add_track', track: { id: ids.track, name: 'VRM立ち絵', kind: 'visual', visible: true, muted: false } },
    { type: 'add_clip', clip: { id: ids.clip, track_id: ids.track, scene_id: sceneId || null,
      start_us: Math.round(start * 1e6), duration_us: Math.round(duration * 1e6),
      transform: { x: (project.canvas.width-width)/2, y: (project.canvas.height-height)/2, width, height, opacity: 1 },
      content: { type: 'vrm', standing: { model_asset_id: ids.model, motion_asset_id: ids.motion,
        source_start_us: 0, expressions: {}, character_id: characterId || null } } } }];
}
