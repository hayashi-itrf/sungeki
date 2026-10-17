<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { Command, Track } from '@sungeki/api-client';
  import type { EditorController } from './controller.svelte';
  let { editor }: { editor: EditorController } = $props();
  const original = (() => editor.snapshot!.project.tracks)();
  let tracks = $state<Track[]>(original.map(t => ({ ...t })));
  function move(index: number, delta: number) {
    const next = index + delta;
    if (next < 0 || next >= tracks.length) return;
    [tracks[index], tracks[next]] = [tracks[next], tracks[index]];
    editor.drafting = true;
  }
  function add() {
    tracks.push({ id: crypto.randomUUID(), name: locale.text('trackNew'), kind: 'visual', visible: true, muted: false });
    editor.drafting = true;
  }
  async function apply(event: SubmitEvent) {
    event.preventDefault();
    const commands: Command[] = tracks.map(track => original.some(t => t.id === track.id)
      ? { type: 'set_track_properties', track_id: track.id, name: track.name, visible: track.visible, muted: track.muted }
      : { type: 'add_track', track: { ...track } });
    commands.push({ type: 'reorder_tracks', track_ids: tracks.map(t => t.id) });
    await editor.apply(commands);
  }
</script>
<form onsubmit={apply} oninput={() => { editor.drafting = true; }}>
  <p>{locale.text('trackHint')}</p>
  <fieldset disabled={editor.busy || !!editor.pending}>
    {#each tracks as track, index (track.id)}
      <section aria-label={locale.text('trackNumber').replace('{number}', String(index + 1))}>
        <label>{locale.text('trackName')}<input aria-label={locale.text('trackNameLabel').replace('{number}', String(index + 1))} bind:value={track.name} /></label>
        <label class="check"><input type="checkbox" bind:checked={track.visible} />{locale.text('trackVisible')}</label>
        <label class="check"><input type="checkbox" bind:checked={track.muted} />{locale.text('trackMute')}</label>
        <button type="button" disabled={index === 0} onclick={() => move(index, -1)}>{locale.text('trackBack')}</button>
        <button type="button" disabled={index === tracks.length - 1} onclick={() => move(index, 1)}>{locale.text('trackFront')}</button>
      </section>
    {/each}
    <button type="button" onclick={add}>{locale.text('trackAdd')}</button>
    <button type="submit">{locale.text('trackApply')}</button>
    {#if editor.drafting}<button type="button" onclick={() => editor.acceptIncoming()}>{locale.text('trackDiscard')}</button>{/if}
  </fieldset>
</form>
<style>
  fieldset { border:0; padding:0; }
  section { display:flex; align-items:end; flex-wrap:wrap; gap:8px; padding:10px; margin:8px 0; background:var(--background); border:1px solid var(--panel-border); border-radius:5px; }
  label { display:flex; flex-direction:column; gap:4px; }
  .check { flex-direction:row; align-self:center; }
  input:not([type=checkbox]) { width:220px; }
  fieldset > button { margin:8px 4px 0 0; }
</style>
