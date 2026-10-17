<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { Project } from '@sungeki/api-client';
  import { clipLabel } from '../editor/clip-label';
  import { duplicateScene, timeUs, type SubmitCommands } from './commands';
  let { project, submit }: { project: Project; submit: SubmitCommands } = $props();
  let selected = $state('');
  let name = $state(locale.text('structureNewScene'));
  let start = $state(0);
  let duration = $state(5);
  let clips = $state<string[]>([]);
  let copyStart = $state(0);
  let layoutStart = $state(0);
  let gap = $state(0);
  let order = $state<string[]>((() => [...project.scenes].sort((a, b) => a.start_us - b.start_us).map(s => s.id))());
  function select() {
    const scene = project.scenes.find(s => s.id === selected);
    name = scene?.name ?? locale.text('structureNewScene'); start = (scene?.start_us ?? 0) / 1_000_000;
    duration = (scene?.duration_us ?? 5_000_000) / 1_000_000;
    clips = project.clips.filter(c => c.scene_id === selected).map(c => c.id);
    copyStart = start + duration;
  }
  function reorder(index: number, delta: number) {
    const next = [...order]; const target = index + delta;
    if (target < 0 || target >= next.length) return;
    [next[index], next[target]] = [next[target]!, next[index]!]; order = next;
  }
  function save() {
    return submit(() => {
      if (!name.trim()) throw new Error(locale.text('structureNameRequired'));
      if (selected) return [
        { type: 'rename_scene', scene_id: selected, name },
        { type: 'move_scene', scene_id: selected, start_us: timeUs(start) },
        { type: 'resize_scene', scene_id: selected, duration_us: timeUs(duration) },
      ];
      const id = crypto.randomUUID();
      return [{ type: 'add_scene', scene: { id, name, start_us: timeUs(start), duration_us: timeUs(duration) } },
        ...clips.map(clip_id => ({ type: 'set_clip_scene' as const, clip_id, scene_id: id }))];
    });
  }
</script>
<section>
  <h3>{locale.text('structureScenes')}</h3>
  <label>{locale.text('structureTarget')} <select bind:value={selected} onchange={select}>
    <option value="">{locale.text('structureNewScene')}</option>
    {#each project.scenes as scene (scene.id)}<option value={scene.id}>{scene.name}（{scene.start_us / 1_000_000}{locale.text('structureSeconds')}）</option>{/each}
  </select></label>
  <form onsubmit={event => { event.preventDefault(); void save(); }}>
    <label>{locale.text('structureName')} <input bind:value={name} required maxlength="200" /></label>
    <label>{locale.text('structureStart')}<input type="number" min="0" max="604800" step="0.000001" bind:value={start} required /></label>
    <label>{locale.text('structureDuration')}<input type="number" min="0.000001" max="604800" step="0.000001" bind:value={duration} required /></label>
    <button>{selected ? locale.text('structureApplyScene') : locale.text('structureCreateScene')}</button>
  </form>
  <details>
    <summary>{locale.text('structureSelectMembers')}</summary>
    {#each project.clips as clip (clip.id)}
      <label><input type="checkbox" bind:group={clips} value={clip.id} />{clipLabel(clip.content)}（{clip.start_us / 1_000_000}{locale.text('structureSeconds')}）</label>
    {/each}
    {#if selected}<button onclick={() => submit(() => project.clips.filter(c => c.scene_id === selected || clips.includes(c.id)).map(c => ({ type: 'set_clip_scene', clip_id: c.id, scene_id: clips.includes(c.id) ? selected : null })))}>{locale.text('structureUpdateMembers')}</button>{/if}
    <p>{locale.text('structureMembersHint')}</p>
  </details>
  {#if selected}
    <label>{locale.text('structureCopyStart')}<input type="number" min="0" step="0.000001" bind:value={copyStart} /></label>
    <button onclick={() => submit(() => [duplicateScene(project, selected, copyStart)])}>{locale.text('structureCopyScene')}</button>
    <button onclick={() => submit(() => [{ type: 'ungroup_scene', scene_id: selected }])}>{locale.text('structureUngroupScene')}</button>
    <button onclick={() => submit(() => [{ type: 'delete_scene', scene_id: selected }])}>{locale.text('structureDeleteScene')}</button>
  {/if}
  <details>
    <summary>{locale.text('structureReorder')}</summary>
    {#each order as id, index (id)}
      <div>{project.scenes.find(s => s.id === id)?.name}（{(project.scenes.find(s => s.id === id)?.start_us ?? 0) / 1_000_000}{locale.text('structureSeconds')}）
        <button disabled={index === 0} onclick={() => reorder(index, -1)}>{locale.text('structureEarlier')}</button>
        <button disabled={index === order.length - 1} onclick={() => reorder(index, 1)}>{locale.text('structureLater')}</button>
      </div>
    {/each}
    <label>{locale.text('structureLayoutStart')}<input type="number" min="0" step="0.000001" bind:value={layoutStart} /></label>
    <label>{locale.text('structureGap')}<input type="number" min="0" step="0.000001" bind:value={gap} /></label>
    <button disabled={!order.length} onclick={() => submit(() => [{ type: 'layout_scenes', scene_ids: order, start_us: timeUs(layoutStart), gap_us: timeUs(gap) }])}>{locale.text('structureApplyOrder')}</button>
    <p>{locale.text('structureUnassignedHint')}</p>
  </details>
</section>
<style>
  section { margin: 16px 0; } label { display: inline-flex; align-items: center; gap: 6px; margin: 6px; font-size: 13px; }
  input[type=number] { width: 110px; } button { padding: 8px 12px; margin: 4px; font-size: 13px; }
  details { margin-top: 12px; } select { max-width: 300px; }
</style>
