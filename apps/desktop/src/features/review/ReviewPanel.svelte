<script lang="ts">
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { locale } from '../../shared/i18n/locale.svelte';
  import { onDestroy } from 'svelte';
  import { hostApi } from '../../shared/host-api';
  import type { EditorController } from '../editor/controller.svelte';
  import { ReviewController, active } from './controller.svelte';
  import { parseSampling } from './sampling';
  let maxFrames = $state<number | undefined>(12), requestedTimes = $state(''), safeMargin = $state<number | undefined>(5);
  function startReview() {
    try { void reviews.start(editor.snapshot!, parseSampling(requestedTimes, maxFrames, safeMargin)); }
    catch (error) { reviews.error = error instanceof Error ? error.message : String(error); }
  }
  import ReviewResult from './ReviewResult.svelte';
  import AssetRepair from './AssetRepair.svelte';
  let { editor }: { editor: EditorController } = $props();
  // The parent keys this panel by session; drafts/retries belong to that session.
  const reviews = (() => new ReviewController(hostApi, editor.snapshot!.session_id))();
  const phases = $derived({ queued: locale.text('reportQueued'), running: locale.text('reportRunning'), cancelling: locale.text('reportCancelling'), cancelled: locale.text('reportCancelled'), succeeded: locale.text('reportSucceeded'), failed: locale.text('reportFailed'), interrupted: locale.text('reportInterrupted') });
  let open = $state(false);
  let repairing = $state<string | null>(null);
  let dialog = $state<HTMLDialogElement>();
  $effect(() => { if (open && dialog) dialog.showModal(); });
  $effect(() => {
    if (!open || !reviews.selected || (!active(reviews.selected) && !reviews.selected.report?.asset_fingerprint)) return;
    const timer = setInterval(() => { if (!reviews.busy && !reviews.pending) void reviews.refresh(); }, 1500);
    return () => clearInterval(timer);
  });
  onDestroy(() => reviews.dispose());
  function reveal(id: string, timeUs?: number) {
    if (reviews.busy || reviews.pending) return;
    if (editor.revealClip(id, timeUs)) open = false;
    else reviews.error = locale.text('reportTargetChanged');
  }
  function revealScene(id: string) {
    if (reviews.busy || reviews.pending) return;
    if (editor.revealScene(id)) open = false;
    else reviews.error = locale.text('reportTargetChanged');
  }
  function repair(id: string) {
    if (reviews.busy || reviews.pending || editor.busy || editor.pending || editor.drafting) return;
    if (!editor.snapshot?.project.assets.some(asset => asset.id === id)) { reviews.error = locale.text('reportAssetMissing'); return; }
    open = false; repairing = id;
  }
  function close() { if (!reviews.busy && !reviews.pending) open = false; }
</script>
<button disabled={editor.busy || editor.drafting || !!editor.pending} onclick={() => { open = true; void reviews.list(); }}>{locale.text('reportTitle')}</button>
{#if open}
  <dialog use:containDialogFocus bind:this={dialog} aria-label={locale.text('reportTitle')} oncancel={e => { e.preventDefault(); close(); }}>
    <header><h2>{locale.text('reportTitle')}</h2><button disabled={reviews.busy || !!reviews.pending} onclick={close}>{locale.text('close')}</button></header>
    <p>{locale.text('reportIntro')}</p>
    <fieldset disabled={reviews.busy || !!reviews.pending}>
      <legend>{locale.text('reportConditions')}</legend>
      <label>{locale.text('reportMargin')}<input type="number" min="0" max="25" step="1" bind:value={safeMargin} /></label>
      <p>{locale.text('reportMarginHint')}</p>
      <label>{locale.text('reportLimit')}<input type="number" min="1" max="120" step="1" bind:value={maxFrames} /></label>
      <label>{locale.text('reportTimes')}<input type="text" bind:value={requestedTimes} placeholder={locale.text('reportExample')} /></label>
      <p>{locale.text('reportSamplingHint')}</p>
    </fieldset>
    <div class="actions">
      <button disabled={reviews.busy || !!reviews.pending || editor.busy || editor.drafting || !!editor.pending} onclick={startReview}>{locale.text('reportStart')}</button>
      <button disabled={reviews.busy || !!reviews.pending} onclick={() => reviews.list()}>{locale.text('refreshList')}</button>
      {#if reviews.selected && active(reviews.selected)}<button disabled={reviews.busy || !!reviews.pending} onclick={() => reviews.cancel()}>{locale.text('reportCancel')}</button>{/if}
    </div>
    {#if reviews.error}<p role="alert" style="white-space: pre-line">{reviews.error}</p>{/if}
    {#if reviews.pending}<p>{locale.text('reportPending')}</p><button disabled={reviews.busy} onclick={() => reviews.retry()}>{locale.text('reportRetryJob')}</button>{/if}
    {#if reviews.selected && reviews.unavailable.some(entry => entry.id === reviews.selected?.job_id)}
      <p role="alert">{locale.text('reportUnavailable')}</p>
    {/if}
    <div class="layout">
      <nav aria-label={locale.text('reportHistory')}>
        <h3>{locale.text('reportHistory')}</h3>
        {#each reviews.jobs as job (job.job_id)}
          <button class:selected={reviews.selected?.job_id === job.job_id} disabled={reviews.busy || !!reviews.pending} onclick={() => reviews.refresh(job.job_id)}>{locale.text('revision')} {job.source_revision} ・ {phases[job.phase]}<small>{job.job_id.slice(0, 8)}{job.stale || job.source_revision !== (editor.incoming?.revision ?? editor.snapshot!.revision) ? locale.text('reportOld') : ''}</small></button>
        {/each}
        {#each reviews.unavailable as entry (entry.id)}
          <div role="alert"><p>{locale.text('reportFetchFailed').replace('{id}', entry.id.slice(0, 8))}</p><small>{entry.message}</small>
            <button disabled={reviews.busy || !!reviews.pending} onclick={() => reviews.refresh(entry.id)}>{locale.text('reportRetryResult')}</button>
          </div>
        {/each}
        {#if !reviews.jobs.length && !reviews.unavailable.length}<p>{reviews.busy ? locale.text('reportLoading') : locale.text('reportEmpty')}</p>{/if}
        {#if reviews.more}<button disabled={reviews.busy || !!reviews.pending} onclick={() => reviews.list(true)}>{locale.text('showMore')}</button>{/if}
      </nav>
      {#if reviews.selected}<ReviewResult job={reviews.selected} revision={editor.incoming?.revision ?? editor.snapshot!.revision} project={editor.snapshot!.project} disabled={reviews.busy || !!reviews.pending || editor.busy || !!editor.pending || editor.drafting} onclip={reveal} onasset={repair} onscene={revealScene} />{:else}<p>{locale.text('reportSelect')}</p>{/if}
    </div>
  </dialog>
{/if}
{#if repairing}<AssetRepair {editor} assetId={repairing} onclose={() => { repairing = null; }} />{/if}
<style>
  header { display:flex; justify-content:space-between; align-items:center; gap:12px; }
  dialog { width:min(980px,95vw); }
  .actions { display:flex; gap:8px; flex-wrap:wrap; margin-bottom:16px; }
  .layout { display:grid; grid-template-columns:220px minmax(0,1fr); gap:16px; }
  nav { border-right:1px solid var(--panel-border); padding-right:12px; }
  nav button { display:block; width:100%; text-align:left; margin-bottom:8px; }
  small { display:block; color:var(--muted); margin-top:4px; }
  .selected { outline:2px solid var(--focus); }
  @media (max-width:760px) { .layout { grid-template-columns:1fr; } nav { border-right:0; border-bottom:1px solid var(--panel-border); } }
</style>
