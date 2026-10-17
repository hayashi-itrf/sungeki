<script lang="ts">
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { locale } from '../../shared/i18n/locale.svelte';
  import { onDestroy } from 'svelte';
  import { hostApi } from '../../shared/host-api';
  import type { EditorController } from '../editor/controller.svelte';
  import EditorNotices from '../editor/EditorNotices.svelte';
  import DictionaryForm from './DictionaryForm.svelte';
  import { PronunciationController } from './pronunciation.svelte';
  let { editor }: { editor: EditorController } = $props();
  const preview = new PronunciationController(hostApi);
  let open = $state(false);
  let dialog = $state<HTMLDialogElement>();
  $effect(() => { if (open && dialog) dialog.showModal(); });
  onDestroy(() => preview.reset());
  function close() {
    if (editor.busy || editor.pending) return;
    editor.acceptIncoming(); preview.reset(); open = false;
  }
</script>
<button disabled={editor.busy || editor.drafting || !!editor.pending} onclick={() => { open = true; }}>{locale.text('pronunciationTitle')}</button>
{#if open && editor.snapshot}
  <dialog use:containDialogFocus bind:this={dialog} aria-label={locale.text('pronunciationTitle')} oncancel={e => { e.preventDefault(); close(); }}>
    <header><h2>{locale.text('pronunciationTitle')}</h2><button disabled={editor.busy || !!editor.pending} onclick={close}>{editor.drafting ? locale.text('pronunciationCancelClose') : locale.text('close')}</button></header>
    <EditorNotices {editor} />
    {#key `${editor.snapshot.session_id}:${editor.snapshot.revision}:${editor.draftGeneration}`}<DictionaryForm {editor} />{/key}
    <section aria-label={locale.text('pronunciationRegion')}>
      <h3>{locale.text('pronunciationRegion')}</h3>
      <p>{locale.text('pronunciationHint')}</p>
      <label>{locale.text('pronunciationSubtitle')}<textarea rows="3" bind:value={preview.subtitle}></textarea></label>
      <label class="option"><input type="checkbox" bind:checked={preview.explicit} />{locale.text('pronunciationExplicit')}</label>
      {#if preview.explicit}<label>{locale.text('pronunciationSpoken')}<textarea rows="3" bind:value={preview.spoken}></textarea></label>{/if}
      {#if editor.drafting}<p role="status">{locale.text('pronunciationDraft')}</p>{/if}
      <button disabled={preview.busy || editor.busy || !!editor.pending || editor.drafting} onclick={() => preview.preview(editor)}>{preview.busy ? locale.text('pronunciationBusy') : locale.text('pronunciationPreview')}</button>
      {#if preview.error}<p role="alert">{preview.error}</p>{/if}
      {#if preview.result}
        <div class="result" aria-label={locale.text('pronunciationResult')}>
          {#if preview.stale(editor)}<p class="stale" role="status">{locale.text('pronunciationStale')}</p>{/if}
          <strong>{locale.text('pronunciationRevision').replace('{revision}', String(preview.result.revision))}</strong>
          <p>{preview.result.explicit_reading ? locale.text('pronunciationDirectResult') : locale.text('pronunciationDictionaryResult')}</p>
          <pre>{preview.result.final_text}</pre>
          {#if preview.result.applied.length}<details><summary>{locale.text('pronunciationReplacements').replace('{count}', String(preview.result.applied.length))}</summary>
            <ul>{#each preview.result.applied as entry}<li>{entry.written} → {entry.spoken} ({locale.text('pronunciationOccurrences').replace('{count}', String(entry.occurrences))})</li>{/each}</ul>
          </details>{:else}<p>{locale.text('pronunciationNoReplacements')}</p>{/if}
        </div>
      {/if}
    </section>
  </dialog>
{/if}
<style>
  header { display:flex; justify-content:space-between; align-items:center; gap:12px; }
  section { margin-top:20px; border-top:2px solid var(--panel-border); padding-top:18px; }
  label { display:flex; flex-direction:column; gap:6px; margin:12px 0; }
  .option { flex-direction:row; align-items:center; }
  textarea { width:100%; }
  .result { border:1px solid var(--panel-border); border-radius:5px; padding:12px; margin-top:14px; background:var(--background); }
  pre { white-space:pre-wrap; overflow-wrap:anywhere; font:inherit; max-height:240px; overflow:auto; }
  .stale { border-left:4px solid var(--accent); padding-left:10px; }
  li { overflow-wrap:anywhere; }
</style>
