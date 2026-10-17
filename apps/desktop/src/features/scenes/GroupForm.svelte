<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { Project } from '@sungeki/api-client';
  import { clipLabel } from '../editor/clip-label';
  import { duplicateGroup, timeUs, type SubmitCommands } from './commands';
  let { project, submit }: { project: Project; submit: SubmitCommands } = $props();
  let selected = $state(''); let name = $state(locale.text('structureNewGroup'));
  let clips = $state<string[]>([]); let start = $state(0); let copyStart = $state(0);
  function select() {
    const group = project.groups.find(g => g.id === selected);
    name = group?.name ?? locale.text('structureNewGroup'); clips = group ? [...group.clip_ids] : [];
    const members = project.clips.filter(c => clips.includes(c.id));
    start = members.length ? Math.min(...members.map(c => c.start_us)) / 1_000_000 : 0;
    copyStart = members.length ? Math.max(...members.map(c => c.start_us + c.duration_us)) / 1_000_000 : 0;
  }
</script>
<section>
  <h3>{locale.text('structureGroups')}</h3>
  <label>{locale.text('structureTarget')} <select bind:value={selected} onchange={select}>
    <option value="">{locale.text('structureNewGroup')}</option>
    {#each project.groups as group (group.id)}<option value={group.id}>{group.name}（{Math.min(...project.clips.filter(c => group.clip_ids.includes(c.id)).map(c => c.start_us)) / 1_000_000}{locale.text('structureSeconds')}）</option>{/each}
  </select></label>
  <label>{locale.text('structureName')} <input bind:value={name} maxlength="200" /></label>
  {#if selected}
    <button onclick={() => submit(() => [{ type: 'rename_group', group_id: selected, name }])}>{locale.text('structureApplyName')}</button>
    <label>{locale.text('structureMoveStart')}<input type="number" min="0" step="0.000001" bind:value={start} /></label>
    <button onclick={() => submit(() => [{ type: 'move_group', group_id: selected, start_us: timeUs(start) }])}>{locale.text('structureMoveGroup')}</button>
    <label>{locale.text('structureCopyStart')}<input type="number" min="0" step="0.000001" bind:value={copyStart} /></label>
    <button onclick={() => submit(() => [duplicateGroup(project, selected, copyStart)])}>{locale.text('structureCopyGroup')}</button>
    <button onclick={() => submit(() => [{ type: 'ungroup', group_id: selected }])}>{locale.text('structureUngroup')}</button>
    <p>{locale.text('structureGroupHint').replace('{count}', String(clips.length))}</p>
  {:else}
    <details open>
      <summary>{locale.text('structureGroupMembers')}</summary>
      {#each project.clips as clip (clip.id)}
        <label><input type="checkbox" bind:group={clips} value={clip.id} />{clipLabel(clip.content)}（{clip.start_us / 1_000_000}{locale.text('structureSeconds')}）</label>
      {/each}
    </details>
    <button disabled={!name.trim() || !clips.length} onclick={() => submit(() => [{ type: 'add_group', group: { id: crypto.randomUUID(), name, clip_ids: clips } }])}>{locale.text('structureCreateGroup')}</button>
  {/if}
</section>
<style>
  section { border-top: 1px solid var(--border); margin-top: 20px; } label { display: inline-flex; align-items: center; gap: 6px; margin: 6px; font-size: 13px; }
  input[type=number] { width: 110px; } button { padding: 8px 12px; margin: 4px; font-size: 13px; } select { max-width: 300px; }
</style>
