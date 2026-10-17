<script lang="ts">
  import type { Clip, Project } from '@sungeki/api-client';
  import { vrmExpressions } from './vrm-expressions';
  import VrmCanvas from './VrmCanvas.svelte';
  let { clip, project, timeUs, sessionId, revision, generation }: {
    clip: Clip; project: Project; timeUs: number; sessionId: string; revision: number; generation: number;
  } = $props();
  let image = $state(''); let status = $state('VRMを読み込んでいます…');
  let t = $derived(clip.transform);
  let source = $derived(project.clips.find(c => c.id === clip.id)?.transform ?? t);
</script>
{#if clip.content.type === 'vrm'}
  <!-- WebKit composites WebGL foreignObjects outside the SVG scale/clip. Keep
       the renderer hidden and composite its transparent frame as an SVG image. -->
  <foreignObject x="0" y="0" width="1" height="1" style="display:none">
    <VrmCanvas standing={vrmExpressions(project, clip.content.standing, timeUs)} assets={project.assets} {sessionId} {revision} {generation}
      timeUs={clip.content.standing.source_start_us + Math.max(0, timeUs - clip.start_us)} width={source.width} height={source.height}
      onImage={png => { image = png; }} onStatus={message => { status = message; }} />
  </foreignObject>
  {#if image && !status}
    <image href={image} aria-label="VRMモデル" x={t.x} y={t.y} width={t.width} height={t.height} preserveAspectRatio="none"
      transform={t.flip_x ? `translate(${2*t.x+t.width} 0) scale(-1 1)` : undefined} />
  {:else}
    <text x={t.x + 8} y={t.y + 24} fill="#65758b" font-size="14">{status}</text>
  {/if}
{/if}
