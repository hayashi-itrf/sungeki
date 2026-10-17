import type { Project } from '@sungeki/api-client';
export function sceneCoverOpacity(project: Project, timeUs: number): number {
  const scene = project.scenes.find(s => s.start_us <= timeUs && timeUs < s.start_us + s.duration_us);
  if (!scene?.fade) return 0;
  const elapsed = timeUs - scene.start_us;
  const fade = scene.fade;
  return 1 - Math.max(0, Math.min(1, fade.in_us ? elapsed / fade.in_us : 1,
    fade.out_us ? (scene.duration_us - elapsed) / fade.out_us : 1));
}
