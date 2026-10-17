<script lang="ts">
  import PreviewLayers from './PreviewLayers.svelte';
  import PreviewVisual from './PreviewVisual.svelte';
  import type { Project } from '@sungeki/api-client';
  import type { PreviewNode } from './preview-layers';
  let { proxies = {}, nodes, project, time, sessionId, revision, generation, prefix }: {
    nodes: PreviewNode[]; proxies?: Readonly<Record<string, string>>; project: Project; time: number; sessionId: string;
    revision: number; generation: number; prefix: string;
  } = $props();
</script>
{#each nodes as node (node.kind === 'background' ? 'background' : node.clip.id)}
  {#if node.kind === 'background'}
    <rect width={project.canvas.width} height={project.canvas.height} fill={project.canvas.background} />
  {:else if node.kind === 'clip'}
    <PreviewVisual clip={node.clip} {project} {time} {sessionId} {revision} {generation} {proxies} />
  {:else}
    {@const t = node.clip.transform}
    {@const id = `${prefix}-${node.clip.id}`}
    <defs>
      <filter {id} filterUnits="userSpaceOnUse" primitiveUnits="userSpaceOnUse" x="0" y="0" width={project.canvas.width} height={project.canvas.height} color-interpolation-filters="sRGB">
        <feGaussianBlur in="SourceGraphic" stdDeviation={node.clip.content.sigma} edgeMode="duplicate" result="blurred" />
        <feFlood x={Math.round(t.x)} y={Math.round(t.y)} width={Math.max(1, Math.round(t.width))} height={Math.max(1, Math.round(t.height))} flood-color="white" flood-opacity={t.opacity} result="mask" />
        <feComposite in="blurred" in2="mask" operator="in" result="region" />
        <feMerge><feMergeNode in="SourceGraphic" /><feMergeNode in="region" /></feMerge>
      </filter>
    </defs>
    <g filter={`url(#${id})`}>
      <PreviewLayers nodes={node.children} {project} {time} {sessionId} {revision} {generation} {proxies} {prefix} />
    </g>
  {/if}
{/each}
