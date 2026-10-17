<script lang="ts">
  import type { Project } from '@sungeki/api-client';
  import { locale } from '../../shared/i18n/locale.svelte';
  import { clipLabel } from '../editor/clip-label';
  import { timeUs, type SubmitCommands } from './commands';
  let { project, selected, submit }: { project: Project; selected: string[]; submit: SubmitCommands } = $props();
  let clips = $state<string[]>([]);
  let boundary = $state(0);
  let seconds = $state(1);
  let direction = $state<'later' | 'earlier'>('later');
  function useSelection() {
    const ids = new Set(selected);
    clips = project.clips.filter(c => ids.has(c.id)).map(c => c.id);
  }
</script>
<details class="ripple">
  <summary>{locale.text('rippleTitle')}</summary>
  <p>{locale.text('rippleHint')}</p>
  <div class="actions">
    <button type="button" disabled={!selected.length} onclick={useSelection}>{locale.text('rippleSelection')}</button>
    <button type="button" onclick={() => { clips = project.clips.map(c => c.id); }}>{locale.text('rippleAll')}</button>
    <button type="button" onclick={() => { clips = []; }}>{locale.text('rippleClear')}</button>
  </div>
  <div class="clips">
    {#each project.clips as clip (clip.id)}
      <label><input type="checkbox" bind:group={clips} value={clip.id} />{clipLabel(clip.content)} ({clip.start_us / 1e6}{locale.text('structureSeconds')})</label>
    {/each}
  </div>
  <p>{locale.text('rippleCount').replace('{count}', String(clips.length))}</p>
  <form onsubmit={event => { event.preventDefault(); void submit(() => [{ type: 'ripple', clip_ids: [...clips], boundary_us: timeUs(boundary), delta_us: timeUs(seconds) * (direction === 'later' ? 1 : -1) }]); }}>
    <label>{locale.text('rippleBoundary')}<input type="number" min="0" max="604800" step="0.000001" bind:value={boundary} required /></label>
    <label>{locale.text('structureDirection')}<select bind:value={direction}><option value="later">{locale.text('structureBackward')}</option><option value="earlier">{locale.text('structureEarlier')}</option></select></label>
    <label>{locale.text('structureDistance')}<input type="number" min="0.000001" max="604800" step="0.000001" bind:value={seconds} required /></label>
    <button disabled={!clips.length}>{locale.text('structureReviewMove')}</button>
  </form>
  <p>{locale.text('rippleConstraint')}</p>
</details>
<style>
  .ripple { border-top: 1px solid var(--border); padding-top: 16px; margin-top: 16px; }
  .actions { display:flex; flex-wrap:wrap; gap:6px; }
  .clips { max-height:240px; overflow:auto; }
  label { display:flex; align-items:center; gap:6px; margin:8px 0; font-size:13px; overflow-wrap:anywhere; }
  input[type=number] { width:110px; } button { padding:8px 12px; }
</style>
