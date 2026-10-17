<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { Project } from '@sungeki/api-client';
  import { clipLabel } from '../editor/clip-label';
  import { timeUs, type SubmitCommands } from './commands';
  let { project, submit }: { project: Project; submit: SubmitCommands } = $props();
  let scenes = $state<string[]>([]); let groups = $state<string[]>([]); let clips = $state<string[]>([]);
  let seconds = $state(1); let direction = $state<'later' | 'earlier'>('later');
  let count = $derived(scenes.length + groups.length + clips.length);
</script>
<details class="selection">
  <summary>{locale.text('structureMoveSelection')}</summary>
  <p>{locale.text('structureOnceHint')}</p>
  <div class="columns">
    <div><h4>{locale.text('structureScenes')}</h4>
      {#each project.scenes as scene (scene.id)}
        <label><input type="checkbox" bind:group={scenes} value={scene.id} />{scene.name}（{scene.start_us / 1_000_000}{locale.text('structureSeconds')}）</label>
      {/each}
    </div>
    <div><h4>{locale.text('structureGroups')}</h4>
      {#each project.groups as group (group.id)}
        <label><input type="checkbox" bind:group={groups} value={group.id} />{group.name}（{locale.text('structureGroupCount').replace('{count}', String(group.clip_ids.length))}・{Math.min(...project.clips.filter(c => group.clip_ids.includes(c.id)).map(c => c.start_us)) / 1_000_000}{locale.text('structureSeconds')}）</label>
      {/each}
    </div>
    <div><h4>{locale.text('structureClips')}</h4>
      {#each project.clips as clip (clip.id)}
        <label><input type="checkbox" bind:group={clips} value={clip.id} />{clipLabel(clip.content)}（{clip.start_us / 1_000_000}{locale.text('structureSeconds')}）</label>
      {/each}
    </div>
  </div>
  <p>{locale.text('structureSelectionCount').replace('{scenes}', String(scenes.length)).replace('{groups}', String(groups.length)).replace('{clips}', String(clips.length))}</p>
  <form onsubmit={event => { event.preventDefault(); void submit(() => [{ type: 'move_selection',
    scene_ids: [...scenes], group_ids: [...groups], clip_ids: [...clips],
    delta_us: timeUs(seconds) * (direction === 'later' ? 1 : -1),
  }]); }}>
    <label>{locale.text('structureDirection')} <select bind:value={direction}><option value="later">{locale.text('structureBackward')}</option><option value="earlier">{locale.text('structureEarlier')}</option></select></label>
    <label>{locale.text('structureDistance')}<input type="number" min="0.000001" max="604800" step="0.000001" bind:value={seconds} required /></label>
    <button disabled={!count}>{locale.text('structureReviewMove')}</button>
  </form>
  <p>{locale.text('structureConflictHint')}</p>
</details>
<style>
  .selection { border-top: 1px solid var(--border); padding-top: 16px; margin-top: 16px; }
  .columns { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; }
  label { display: flex; align-items: center; gap: 6px; margin: 8px 0; font-size: 13px; overflow-wrap: anywhere; }
  input[type=number] { width: 110px; } button { padding: 8px 12px; }
  @media (max-width: 700px) { .columns { grid-template-columns: 1fr; } }
</style>
