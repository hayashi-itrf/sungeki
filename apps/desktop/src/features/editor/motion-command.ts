import type { Clip, Command, VisualMotion } from '@sungeki/api-client';
export function motionCommand(clip: Clip, motion: VisualMotion | undefined): Command {
  return { type: 'set_transform', clip_id: clip.id, transform: { ...clip.transform, motion } };
}
