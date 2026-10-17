import type { Command, Project } from '@sungeki/api-client';
import { clipLabel } from '../editor/clip-label';

/** English descriptions for structure edits; names remain project content. */
export function describeStructure(command: Command, project: Project): string | null {
  const scene = (id: string) => {
    const value = project.scenes.find(s => s.id === id);
    return value ? `“${value.name}” (${value.start_us / 1e6} s)` : 'the selected scene';
  };
  const group = (id: string) => `“${project.groups.find(g => g.id === id)?.name ?? 'Selected group'}”`;
  switch (command.type) {
    case 'move_selection': return `Move scenes ${command.scene_ids.map(scene).join(', ') || 'none'}, groups ${command.group_ids.map(group).join(', ') || 'none'} and ${command.clip_ids.length} individual clips ${Math.abs(command.delta_us) / 1e6} s ${command.delta_us < 0 ? 'earlier' : 'later'}. Each clip moves once.`;
    case 'add_scene': return `Create scene “${command.scene.name}” at ${command.scene.start_us / 1e6} s for ${command.scene.duration_us / 1e6} s.`;
    case 'rename_scene': return `Rename scene ${scene(command.scene_id)} to “${command.name}”.`;
    case 'move_scene': return `Move scene ${scene(command.scene_id)} and its clips to ${command.start_us / 1e6} s.`;
    case 'resize_scene': return `Set scene ${scene(command.scene_id)} duration to ${command.duration_us / 1e6} s.`;
    case 'delete_scene': return `Delete scene ${scene(command.scene_id)}, its clips and its groups.`;
    case 'ungroup_scene': return `Remove only scene ${scene(command.scene_id)}; keep its clips at their current times.`;
    case 'duplicate_scene': return `Copy scene ${scene(command.scene_id)} to ${command.start_us / 1e6} s with ${Object.keys(command.clip_ids).length} clips and ${Object.keys(command.group_ids).length} groups.`;
    case 'layout_scenes': return `Arrange ${command.scene_ids.map(scene).join(' → ')} from ${command.start_us / 1e6} s with gaps of ${command.gap_us / 1e6} s.`;
    case 'add_group': return `Create group “${command.group.name}” with ${command.group.clip_ids.length} clips.`;
    case 'rename_group': return `Rename group ${group(command.group_id)} to “${command.name}”.`;
    case 'move_group': return `Move group ${group(command.group_id)} to ${command.start_us / 1e6} s.`;
    case 'duplicate_group': return `Copy group ${group(command.group_id)} to ${command.start_us / 1e6} s with ${Object.keys(command.clip_ids).length} clips.`;
    case 'ungroup': return `Remove only group ${group(command.group_id)}; keep its clips.`;
    case 'set_clip_scene': {
      const clip = project.clips.find(c => c.id === command.clip_id);
      return `Assign clip “${clip ? clipLabel(clip.content) : 'Selected clip'}” to ${command.scene_id ? scene(command.scene_id) : 'no scene'}.`;
    }
    default: return null;
  }
}
