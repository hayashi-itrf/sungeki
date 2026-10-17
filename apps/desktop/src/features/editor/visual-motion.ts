import type { Transform } from '@sungeki/api-client';
export function motionTransform(start: Transform, elapsed: number, duration: number): Transform {
  const motion = start.motion;
  if (!motion) return start;
  let p = Math.max(0, Math.min(1, elapsed / duration));
  if (motion.easing === 'ease_in_out') p = 0.5 - 0.5 * Math.cos(Math.PI * p);
  const scale = 1 + (motion.end_scale - 1) * p;
  return { ...start, motion: undefined, x: start.x + (motion.end_x - start.x) * p,
    y: start.y + (motion.end_y - start.y) * p, width: start.width * scale, height: start.height * scale };
}
