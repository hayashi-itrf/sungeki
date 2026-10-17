import { locale } from '../../shared/i18n/locale.svelte';
import type { ClipContent, Command, Project } from '@sungeki/api-client';
export type SpriteContent = Extract<ClipContent, { type: 'sprite' }>;

// Only prepare the editor transaction. The host validates all pose dependencies.
export function spriteCommands(project: Project, content: SpriteContent, start: number,
  duration: number, sceneId: string, ids: { clip: string; track: string }): Command[] {
  if (![start, duration].every(Number.isFinite) || start < 0 || duration <= 0
    || start + duration > 604800 || Math.round(duration * 1e6) === 0) {
    throw new Error(locale.text('standingTimeError'));
  }
  const pose = content.sprites.poses.find(p => p.name === (content.pose ?? content.sprites.default_pose));
  const asset = project.assets.find(a => a.id === pose?.closed_asset_id);
  if (asset?.probe.type !== 'image') throw new Error(locale.text('spriteChooseClosed'));
  const { width, height } = asset.probe.info;
  const scale = Math.min(1, project.canvas.width / width, project.canvas.height / height);
  return [
    { type: 'add_track', track: { id: ids.track, name: locale.text('spriteStanding'), kind: 'visual', visible: true, muted: false } },
    { type: 'add_clip', clip: { id: ids.clip, track_id: ids.track, scene_id: sceneId || null,
      start_us: Math.round(start * 1e6), duration_us: Math.round(duration * 1e6),
      transform: { x: (project.canvas.width - width * scale) / 2,
        y: (project.canvas.height - height * scale) / 2, width: width * scale, height: height * scale, opacity: 1 },
      content: { ...content, sprites: { ...content.sprites, poses: content.sprites.poses.map(p => ({ ...p })) } },
    } },
  ];
}
