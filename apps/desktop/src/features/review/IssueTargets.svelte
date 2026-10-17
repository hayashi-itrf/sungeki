<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { Issue, Project } from '@sungeki/api-client';
  import { assetUses, issueTime } from './targets';
  import { timeLabel } from '../editor/timeline-layout';
  let { issue, project, disabled = false, stale = false, onclip, onasset, onscene }: {
    issue: Issue; project: Project; disabled?: boolean; stale?: boolean;
    onclip: (id: string, timeUs?: number) => void; onasset: (id: string) => void; onscene?: (id: string) => void;
  } = $props();
  let limit = $state(8);
  const scene = $derived(project.scenes.find(scene => scene.id === issue.scene_id));
  const asset = $derived(project.assets.find(asset => asset.id === issue.asset_id));
  const clip = $derived(project.clips.find(clip => clip.id === issue.clip_id));
  const uses = $derived(asset ? assetUses(project, asset.id) : []);
  const time = $derived(issueTime(issue, clip, stale));
</script>
{#if issue.clip_id}
  {#if clip}
    {#if time !== undefined}<button {disabled} onclick={() => onclip(clip.id, time)}>{locale.text('reviewTargetTime').replace('{time}', timeLabel(time / 1_000_000))}</button>{/if}
    <button {disabled} onclick={() => onclip(clip.id)}>{locale.text('reviewTargetClip').replace('{time}', timeLabel(clip.start_us / 1_000_000))}</button>
  {:else}<p>{locale.text('reviewTargetMissingClip')}</p>{/if}
{:else if issue.asset_id}
  {#if asset}
    <p>{locale.text('reviewTargetAsset')}{asset.name}</p>
    <button {disabled} onclick={() => onasset(asset.id)}>{locale.text('reviewTargetRepair')}</button>
    {#if uses.length}
      <details><summary>{locale.text('reviewTargetUses').replace('{count}', String(uses.length))}</summary>
        {#each uses.slice(0, limit) as clip (clip.id)}
          <button {disabled} onclick={() => onclip(clip.id)}>{locale.text('reviewTargetLocation').replace('{track}', project.tracks.find(track => track.id === clip.track_id)?.name ?? locale.text('reviewTargetTrack')).replace('{time}', timeLabel(clip.start_us / 1_000_000))}</button>
        {/each}
        {#if uses.length > limit}<button onclick={() => limit += 8}>{locale.text('reviewTargetMore')}</button>{/if}
      </details>
    {:else}<p>{locale.text('reviewTargetUnused')}</p>{/if}
  {:else}<p>{locale.text('reviewTargetMissingAsset')}</p>{/if}
{/if}
{#if issue.scene_id && onscene}
  {#if scene}
    <button {disabled} onclick={() => onscene?.(scene.id)}>{locale.text(stale ? 'reviewTargetCurrentScene' : 'reviewTargetScene').replace('{name}', scene.name).replace('{time}', timeLabel(scene.start_us / 1_000_000))}</button>
  {:else}<p>{locale.text('reviewTargetMissingScene')}</p>{/if}
{/if}
<style>
  p { margin:8px 0; }
  button { margin:4px 6px 4px 0; }
  details { margin-top:8px; }
  details button { display:block; }
</style>
