<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { EditorController } from '../editor/controller.svelte';
  import SpeechFields from './SpeechFields.svelte';
  import { nextSpeechStart, speechCommands } from './create-speech';
  let { editor, submitting }: { editor: EditorController; submitting: (id: string) => void } = $props();
  const project = (() => editor.snapshot!.project)();
  let characterId = $state(project.characters[0]?.id ?? '');
  let subtitle = $state('');
  let spoken = $state('');
  let explicit = $state(false);
  let trackId = $state(project.tracks.find(t => t.kind === 'audio' && t.visible && !t.muted)?.id ?? '');
  let sceneId = $state('');
  let start = $state((() => editor.time)());
  let duration = $state(5);
  const scene = $derived(project.scenes.find(s => s.id === sceneId));
  const followingStart = $derived(nextSpeechStart(project, trackId));
  async function add(event: SubmitEvent) {
    event.preventDefault();
    if (editor.busy || editor.pending) return;
    try {
      const result = speechCommands(project, { characterId, subtitle, spoken, explicit, trackId, sceneId, start, duration });
      submitting(result.clipId);
      await editor.apply(result.commands);
    } catch (error) { editor.error = error instanceof Error ? error.message : String(error); }
  }
</script>
<form onsubmit={add} oninput={() => { editor.drafting = true; }}>
  {#if !project.characters.length}
    <p role="status">{locale.text('speechNoCharacter')}</p>
  {/if}
  <fieldset disabled={editor.busy || !!editor.pending || !project.characters.length}>
    <SpeechFields characters={project.characters} bind:characterId bind:subtitle bind:spoken bind:explicit />
    <div class="placement">
      <label>{locale.text('speechStartSeconds')}<input type="number" min="0" max="604800" step="0.000001" required bind:value={start} /></label>
      <label>{locale.text('speechTemporaryDuration')}<input type="number" min="0.000001" max="604800" step="0.000001" required bind:value={duration} /></label>
    </div>
    <p>{locale.text('speechTemporaryHint')}</p>
    <label>{locale.text('speechAudioTrack')}<select bind:value={trackId}>
      <option value="">{locale.text('speechNewTrack')}</option>
      {#each project.tracks.filter(t => t.kind === 'audio') as track}<option value={track.id}>{track.name}{track.visible ? '' : locale.text('speechHiddenSuffix')}{track.muted ? locale.text('speechMutedSuffix') : ''}</option>{/each}
    </select></label>
    <button type="button" disabled={followingStart === null} onclick={() => {
      if (followingStart !== null) { start = followingStart; editor.drafting = true; }
    }}>{locale.text('speechAfterLast')}</button>
    <p>{locale.text('speechAfterLastHint')}</p>
    <label>{locale.text('speechScene')}<select bind:value={sceneId}><option value="">{locale.text('speechNoScene')}</option>{#each project.scenes as scene}<option value={scene.id}>{scene.name}</option>{/each}</select></label>
    {#if scene}<p>{locale.text('speechSceneRange').replace('{start}', String(scene.start_us / 1e6)).replace('{end}', String((scene.start_us + scene.duration_us) / 1e6))}</p>{/if}
    <button type="submit">{locale.text('speechAddTitle')}</button>
  </fieldset>
</form>
<style>
  fieldset { border:0; padding:0; }
  label { display:flex; flex-direction:column; gap:5px; margin:12px 0; }
  input, select { width:100%; }
  .placement { display:grid; grid-template-columns:1fr 1fr; gap:12px; }
  p { font-size:12px; }
</style>
