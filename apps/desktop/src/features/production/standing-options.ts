import type { Project } from '@sungeki/api-client';

function stable(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stable).join(',')}]`;
  if (value !== null && typeof value === 'object') {
    return `{${Object.entries(value).sort(([a], [b]) => a.localeCompare(b)).map(([k, v]) => `${JSON.stringify(k)}:${stable(v)}`).join(',')}}`;
  }
  return JSON.stringify(value) ?? 'null';
}

// Compare only the values carried into a new recipe, not source clip timing.
export function standingOptions(project: Project, character: string) {
  if (!character) return [];
  const seen = new Set<string>();
  return project.clips.filter(clip => {
    const content = clip.content;
    if (content.type !== 'image' && content.type !== 'psd' && content.type !== 'sprite' && content.type !== 'vrm') return false;
    if (content.type === 'psd' && content.speech?.character_id !== character) return false;
    if (content.type === 'vrm' && content.standing.character_id !== character) return false;
    if (content.type === 'sprite' && content.character_id !== character) return false;
    const key = stable({ content, flip_x: clip.transform.flip_x ?? false });
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}
