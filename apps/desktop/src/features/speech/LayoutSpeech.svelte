<script lang="ts">
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { SessionSnapshot } from '@sungeki/api-client';
  import type { EditorController } from '../editor/controller.svelte';
  import { hostApi } from '../../shared/host-api';
  import { EditPlan } from '../scenes/edit-plan.svelte';
  import EditReview from '../scenes/EditReview.svelte';
  import EditorNotices from '../editor/EditorNotices.svelte';
  let { editor }: { editor: EditorController } = $props();
  let dialog: HTMLDialogElement;
  let observed = $state.raw<SessionSnapshot | null>(null);
  let selected = $state<string[]>([]);
  let start = $state(0);
  let gap = $state(0.35);
  const plan = new EditPlan(hostApi);
  let speeches = $derived(observed?.project.clips.filter(c => c.content.type === 'speech')
    .sort((a, b) => a.start_us - b.start_us || a.id.localeCompare(b.id)) ?? []);
  let generated = $derived(speeches.filter(c => c.content.type === 'speech' && c.content.audio));
  let stale = $derived(!!observed && (observed.session_id !== editor.snapshot?.session_id || observed.revision !== editor.snapshot?.revision));
  let disabled = $derived(stale || editor.busy || !!editor.pending || editor.drafting);
  function refresh() {
    plan.cancel(); observed = editor.snapshot;
    selected = speeches.filter(c => c.content.type === 'speech' && c.content.audio).map(c => c.id);
    start = (speeches.find(c => selected.includes(c.id))?.start_us ?? 0) / 1_000_000;
  }
  async function preview(event: SubmitEvent) {
    event.preventDefault();
    if (disabled || !observed || !selected.length) return;
    await plan.prepare(observed, [{ type: 'layout_speech',
      clip_ids: speeches.filter(c => selected.includes(c.id)).map(c => c.id),
      start_us: Math.round(start * 1_000_000), gap_us: Math.round(gap * 1_000_000) }]);
  }
  async function confirm() {
    if (disabled || plan.busy || plan.stale(editor.snapshot) || !plan.request || !plan.preview || plan.error) return;
    await editor.request(plan.request);
    if (!editor.error && !editor.pending) { plan.cancel(); dialog.close(); }
  }
</script>
<button disabled={editor.busy || !!editor.pending || editor.drafting} onclick={() => { refresh(); dialog.showModal(); }}>{locale.text('speechLayoutTitle')}</button>
<dialog use:containDialogFocus bind:this={dialog} onclose={() => plan.cancel()} aria-label={locale.text('speechLayoutTitle')}>
  <header><h2>{locale.text('speechLayoutTitle')}</h2><button onclick={() => dialog.close()}>{locale.text('close')}</button></header>
  <EditorNotices {editor} />
  <p>{locale.text('speechLayoutDescription')}</p>
  <p>{locale.text('speechLayoutScope')}</p>
  {#if stale}<p role="alert">{locale.text('speechLayoutStale')}</p>{/if}
  <button disabled={editor.busy || !!editor.pending} onclick={refresh}>{locale.text('speechLayoutRefresh')}</button>
  {#if observed && plan.request}
    <EditReview {plan} project={observed.project} stale={stale || plan.stale(editor.snapshot)} {disabled} {confirm} />
  {:else}
    <form onsubmit={preview}>
      <fieldset disabled={disabled}>
        <legend>{locale.text('speechLayoutLines')}</legend>
        <div class="selection-actions">
          <button type="button" disabled={!generated.length} onclick={() => { selected = generated.map(c => c.id); }}>{locale.text('speechLayoutAll')}</button>
          <button type="button" disabled={!selected.length} onclick={() => { selected = []; }}>{locale.text('speechLayoutClear')}</button>
        </div>
        <p role="status">{locale.text('speechLayoutCount').replace('{count}', String(selected.length))}</p>
        {#if selected.length > 200}<p role="alert">{locale.text('speechLayoutLimit')}</p>{/if}
        {#if !speeches.length}<p>{locale.text('speechLayoutEmpty')}</p>
        {:else if !generated.length}<p>{locale.text('speechLayoutGenerate')}</p>{/if}
        <div class="lines">
          {#each speeches as clip (clip.id)}
            {#if clip.content.type === 'speech'}
              <label><input type="checkbox" value={clip.id} bind:group={selected} disabled={!clip.content.audio} />
                {clip.start_us / 1_000_000} {locale.text('secondsUnit')}: {clip.content.subtitle_text}{!clip.content.audio ? locale.text('speechLayoutMissing') : ''}</label>
            {/if}
          {/each}
        </div>
        <label>{locale.text('speechLayoutStart')}<input type="number" min="0" max="604800" step="0.000001" required bind:value={start} /></label>
        <label>{locale.text('speechLayoutGap')}<input type="number" min="0" max="604800" step="0.000001" required bind:value={gap} /></label>
        <button disabled={!selected.length || selected.length > 200}>{locale.text('speechLayoutReview').replace('{count}', String(selected.length))}</button>
      </fieldset>
    </form>
  {/if}
</dialog>
<style>
  .selection-actions { display:flex; flex-wrap:wrap; gap:8px; margin:12px 0; }
  header { display:flex; justify-content:space-between; align-items:center; gap:20px; }
  dialog { max-width:760px; }
  fieldset { border:0; padding:12px 0; }
  label { display:block; margin:10px 0; }
  .lines { max-height:240px; overflow:auto; border:1px solid var(--border); padding:8px; }
</style>
