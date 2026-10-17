<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { Project } from '@sungeki/api-client';
  import { selectDiagram, type DiagramInput } from './diagrams';
  import BuildReactionFields from './BuildReactionFields.svelte';
  import BuildDiagramFade from './BuildDiagramFade.svelte';
  import BuildDiagramMotion from './BuildDiagramMotion.svelte';
  import { newDiagramMotion } from './diagram-motion';
  import { newReaction } from './reaction';
  import VideoFields from '../video/VideoFields.svelte';
  let { project, row = $bindable(), index, count, portrait = false }: { project: Project; row: DiagramInput; index: number; count: number; portrait?: boolean } = $props();
  let error = $state('');
  const asset = $derived(project.assets.find(a => a.id === row.asset));
  function select(id: string) {
    error = '';
    try { row = selectDiagram(project, row, id); }
    catch (cause) { row = { asset: id, first: row.first, last: row.last }; error = cause instanceof Error ? cause.message : String(cause); }
  }
</script>
<label>{locale.text('effectsMedia')}<select required value={row.asset} onchange={e => select(e.currentTarget.value)}>
  <option value="">{locale.text('effectsChoose')}</option>
  {#each project.assets.filter(a => a.probe.type === 'image' || a.probe.type === 'video') as item}<option value={item.id}>{item.name}{item.probe.type === 'video' ? locale.text('effectsVideo') : ''}</option>{/each}
</select></label>
<label>{locale.text('effectsFirst')}<input type="number" required min="1" max={count} step="1" bind:value={row.first} /></label>
<label>{locale.text('effectsLast')}<input type="number" required min="1" max={count} step="1" bind:value={row.last} /></label>
{#if asset?.probe.type === 'image'}
  <label>{locale.text('effectsDisplay')}<select value={row.reaction ? 'reaction' : row.motion ? 'motion' : 'diagram'} onchange={e => { if (e.currentTarget.value === 'reaction') row.fade = undefined; row.reaction = e.currentTarget.value === 'reaction' ? newReaction(portrait) : undefined; row.motion = e.currentTarget.value === 'motion' ? newDiagramMotion() : undefined; }}>
    <option value="diagram">{locale.text('effectsDiagram')}</option><option value="motion">{locale.text('effectsMotion')}</option><option value="reaction">{locale.text('effectsReaction')}</option>
  </select></label>
  {#if row.motion}<BuildDiagramMotion bind:motion={row.motion} />{/if}
  {#if row.reaction}<BuildReactionFields {portrait} bind:reaction={row.reaction} />{/if}
{/if}
{#if asset && !row.reaction}<BuildDiagramFade bind:fade={row.fade} />{/if}
{#if error}<p role="alert">{locale.text('effectsItem')} {index + 1}：{error}</p>{/if}
{#if asset?.probe.type === 'video' && row.videoPlayback}
  <VideoFields probe={asset.probe.info} bind:playback={row.videoPlayback} production />
{/if}
<style>
  label { display:grid; gap:6px; margin:8px 0; } select,input { width:100%; }
  [role="alert"] { color:var(--danger); }
</style>
