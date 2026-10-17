<script lang="ts">
  import type { Clip, Project, Transform } from '@sungeki/api-client';
  import { spritePreview } from './sprite-preview';
  import PreviewImage from '../editor/PreviewImage.svelte';
  let { project, clipId, timeUs, sessionId, revision, generation, transform: t }: {
    project: Project; clipId: string; timeUs: number; sessionId: string; revision: number;
    generation: number; transform: Transform;
  } = $props();
  // Use the immutable original, not the per-time transformed preview copy.
  let clip = $derived(project.clips.find(c => c.id === clipId) as Clip | undefined);
  let plan = $derived(clip ? spritePreview(project, clip) : null);
  let assetId = $derived(plan?.at(timeUs));
</script>
{#if plan?.error}
  <g><title>{plan.error}</title>
    <rect x={t.x} y={t.y} width={t.width} height={t.height} fill="none" stroke="#65758b" stroke-dasharray="8 8" />
    <text x={t.x + 8} y={t.y + 24} fill="#65758b" font-size="20">{plan.error}</text>
  </g>
{:else if assetId}
  <PreviewImage {sessionId} {revision} {generation} {assetId} transform={t} />
{/if}
