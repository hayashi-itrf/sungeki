import { locale } from '../../shared/i18n/locale.svelte';
import type { Project } from '@sungeki/api-client';
export function audioLabel(project: Project, id: string): string {
  const asset = project.assets.find(a => a.id === id);
  if (!asset) return locale.text('loudnessMissing');
  const clips = project.clips.filter(c => c.content.type === 'speech' && c.content.audio?.asset_id === id);
  const clip = clips[0];
  if (!clip || clip.content.type !== 'speech') return asset.name;
  const text = clip.content.subtitle_text.replace(/\s+/g, ' ').trim();
  const title = text.length > 45 ? `${text.slice(0, 45)}…` : text || asset.name;
  const time = locale.text('loudnessTime').replace('{seconds}', (clip.start_us / 1_000_000).toFixed(2));
  const shared = clips.length > 1 ? locale.text('loudnessShared').replace('{count}', String(clips.length)) : '';
  return `${time}${title}${shared}`;
}
