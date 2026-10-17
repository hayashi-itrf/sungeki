import { locale } from '../../shared/i18n/locale.svelte';
import type { Command, Project } from '@sungeki/api-client';
export function timeUs(seconds: number): number {
  if (!Number.isFinite(seconds) || seconds < 0 || seconds > 604800) throw new Error(locale.text('structureTimeInvalid'));
  return Math.round(seconds * 1_000_000);
}
export function duplicateScene(project: Project, sceneId: string, start: number): Command {
  const clips = project.clips.filter(c => c.scene_id === sceneId);
  const ids = new Set(clips.map(c => c.id));
  return { type: 'duplicate_scene', scene_id: sceneId, new_scene_id: crypto.randomUUID(), start_us: timeUs(start),
    clip_ids: Object.fromEntries(clips.map(c => [c.id, crypto.randomUUID()])),
    group_ids: Object.fromEntries(project.groups.filter(g => g.clip_ids.some(id => ids.has(id))).map(g => [g.id, crypto.randomUUID()])),
  };
}
export function duplicateGroup(project: Project, groupId: string, start: number): Command {
  const group = project.groups.find(g => g.id === groupId);
  if (!group) throw new Error(locale.text('structureGroupRequired'));
  return { type: 'duplicate_group', group_id: groupId, new_group_id: crypto.randomUUID(), start_us: timeUs(start),
    clip_ids: Object.fromEntries(group.clip_ids.map(id => [id, crypto.randomUUID()])),
  };
}
export type SubmitCommands = (build: () => Command[]) => Promise<void>;
