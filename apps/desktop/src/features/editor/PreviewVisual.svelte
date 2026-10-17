<script lang="ts">
  import VrmPreview from '../standing/VrmPreview.svelte';
  import SpritePreview from '../standing/SpritePreview.svelte';
  import SubtitlePreview from './SubtitlePreview.svelte';
  import PreviewImage from './PreviewImage.svelte';
  import type { Clip, Project } from '@sungeki/api-client';
  let { proxies = {}, clip, project, time, sessionId, revision, generation }: { clip: Clip; proxies?: Readonly<Record<string, string>>; project: Project; time: number; sessionId: string; revision: number; generation: number } = $props();
  let t = $derived(clip.transform);
</script>
      <g opacity={t.opacity}>
        {#if clip.content.type === 'shape'}
          {#if clip.content.shape === 'ellipse'}
            <ellipse cx={t.x + t.width / 2} cy={t.y + t.height / 2} rx={t.width / 2} ry={t.height / 2} fill={clip.content.fill} />
          {:else}
            <rect x={t.x} y={t.y} width={t.width} height={t.height} fill={clip.content.fill} />
          {/if}
        {:else if clip.content.type === 'vrm'}
          <VrmPreview {clip} {project} timeUs={Math.round(time*1e6)} {sessionId} {revision} {generation} />
        {:else if clip.content.type === 'sprite'}
          <SpritePreview {project} clipId={clip.id} timeUs={Math.round(time * 1e6)} {sessionId} {revision} {generation} transform={t} />
        {:else if clip.content.type === 'psd'}
          <PreviewImage {sessionId} {revision} {generation} assetId={clip.content.asset_id} clipId={clip.id} timeUs={Math.round(time * 1_000_000)} transform={t} />
        {:else if clip.content.type === 'video'}
          <PreviewImage {sessionId} {revision} {generation} video={true} proxyJobId={proxies[clip.content.asset_id]} assetId={clip.content.asset_id} clipId={clip.id} timeUs={Math.round(time * 1_000_000)} transform={t} />
        {:else if clip.content.type === 'image'}
          <PreviewImage {sessionId} {revision} {generation} assetId={clip.content.asset_id} transform={t} />
        {:else if clip.content.type === 'speech'}
          {@const characterId = clip.content.character_id}
          {@const character = project.characters.find(c => c.id === characterId)}
          {#if character}
            <SubtitlePreview text={clip.content.subtitle_text} style={character.subtitle_style} transform={t} />
          {/if}
        {:else if clip.content.type === 'text'}
            <SubtitlePreview text={clip.content.text} style={{ font_size: clip.content.font_size, color: clip.content.color, outline: clip.content.outline ?? null, layout: clip.content.layout ?? null }} transform={t} />
        {/if}
      </g>
