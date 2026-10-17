<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { EditorController } from '../editor/controller.svelte';
  let { editor }: { editor: EditorController } = $props();
  // Parent remounts only after a confirmed revision or explicit draft discard.
  let entries = $state((() => editor.snapshot!.project.pronunciation_dictionary.map(entry => ({ ...entry })))());
  let offset = $state(0);
  const start = $derived(Math.min(offset, Math.max(0, Math.ceil(entries.length / 20) - 1) * 20));
  const locked = $derived(editor.busy || !!editor.pending);
  function add() { entries.push({ written: '', spoken: '' }); offset = Math.floor((entries.length - 1) / 20) * 20; editor.drafting = true; }
  function remove(index: number) { entries.splice(index, 1); editor.drafting = true; }
  async function apply(event: SubmitEvent) {
    event.preventDefault();
    if (locked) return;
    await editor.apply([{ type: 'set_pronunciation_dictionary', entries: entries.map(({ written, spoken }) => ({ written, spoken })) }]);
  }
</script>
<form onsubmit={apply} oninput={() => { editor.drafting = true; }}>
  <h3>{locale.text('dictionaryTitle')}</h3>
  <p>{locale.text('dictionaryHint')}</p>
  <fieldset disabled={locked}>
    {#each entries.slice(start, start + 20) as entry, index}
      <div class="entry">
        <label>{locale.text('dictionaryWritten')} {start + index + 1}<input required bind:value={entry.written} /></label>
        <label>{locale.text('dictionarySpoken')} {start + index + 1}<input required bind:value={entry.spoken} /></label>
        <button type="button" aria-label={locale.text('dictionaryRemoveLabel').replace('{index}', String(start + index + 1))} onclick={() => remove(start + index)}>{locale.text('dictionaryDelete')}</button>
      </div>
    {:else}<p>{locale.text('dictionaryEmpty')}</p>{/each}
    {#if entries.length >= 1024}<p role="status">{locale.text('dictionaryLimit')}</p>{/if}
    <div class="actions">
      <button type="button" disabled={entries.length >= 1024} onclick={add}>{locale.text('dictionaryAdd')}</button>
      {#if entries.length > 20}<button type="button" disabled={start === 0} onclick={() => { offset = start - 20; }}>{locale.text('dictionaryPrevious')}</button>
        <span>{locale.text('dictionaryRange').replace('{start}', String(start + 1)).replace('{end}', String(Math.min(start + 20, entries.length))).replace('{count}', String(entries.length))}</span>
        <button type="button" disabled={start + 20 >= entries.length} onclick={() => { offset = start + 20; }}>{locale.text('dictionaryNext')}</button>{/if}
      <button type="submit" disabled={!editor.drafting}>{locale.text('dictionaryApply')}</button>
      {#if editor.drafting}<button type="button" onclick={() => editor.acceptIncoming()}>{locale.text('dictionaryDiscard')}</button>{/if}
    </div>
  </fieldset>
  <small>{locale.text('dictionarySaveHint')}</small>
</form>
<style>
  fieldset { border:0; padding:0; margin:0; min-width:0; }
  .entry { display:grid; grid-template-columns:1fr 1fr auto; align-items:end; gap:8px; margin:10px 0; }
  label { display:flex; flex-direction:column; gap:4px; min-width:0; }
  input { width:100%; }
  .actions { display:flex; flex-wrap:wrap; align-items:center; gap:8px; margin:12px 0; }
  small { color:var(--muted); }
</style>
