<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { sceneCoverOpacity } from './scene-fade';
  import PreviewLayers from './PreviewLayers.svelte';
  import { previewLayers } from './preview-layers';
  import { previewClips } from './preview-evaluation';
  import type { Project } from '@sungeki/api-client';
  let { proxies = {}, project, time, sessionId, revision, generation }: { proxies?: Readonly<Record<string, string>>; project: Project; time: number; sessionId: string; revision: number; generation: number } = $props();
  let clips = $derived(previewClips(project, Math.round(time * 1_000_000)));
  let layers = $derived(previewLayers(clips));
  let cover = $derived(sceneCoverOpacity(project, Math.round(time * 1_000_000)));
  const videoSources = $derived(clips.flatMap(clip => {
    if (clip.content.type !== 'video') return [];
    const assetId = clip.content.asset_id;
    const name = project.assets.find(a => a.id === assetId)?.name ?? locale.text('clipVideo');
    return [`${name}：${proxies[assetId] ? locale.text('previewProxy') : locale.text('previewOriginal')}`];
  }));
  const prefix = `preview-${crypto.randomUUID()}`;
</script>
<div class="preview">
  <svg viewBox={`0 0 ${project.canvas.width} ${project.canvas.height}`} role="img" aria-label={locale.text('previewAccessible')}>
    <defs>
      <clipPath id={`${prefix}-canvas`} clipPathUnits="userSpaceOnUse">
        <rect x="0" y="0" width={project.canvas.width} height={project.canvas.height} />
      </clipPath>
    </defs>
    <!-- The SVG viewport can include letterboxing outside the project viewBox. -->
    <g clip-path={`url(#${prefix}-canvas)`}>
      <PreviewLayers nodes={layers} {project} {time} {sessionId} {revision} {generation} {proxies} {prefix} />
      {#if cover > 0}<rect x="0" y="0" width={project.canvas.width} height={project.canvas.height} fill={project.canvas.background} opacity={cover} />{/if}
    </g>
  </svg>
  {#if project.clips.some(c => c.content.type === 'video')}<small role="status">{videoSources.length ? `${videoSources.join(' ／ ')}。` : ''}{locale.text('previewVideoHint')}</small>{/if}
  <small>{locale.text('previewSimpleHint')}</small>
</div>
<style>
  .preview { flex:1; min-height:0; display:flex; flex-direction:column; justify-content:center; gap:6px; }
  svg { width: 100%; min-height:0; flex:1; display: block; border: 1px solid var(--border); }
  small { color: var(--muted); font-size:11px; }
</style>
