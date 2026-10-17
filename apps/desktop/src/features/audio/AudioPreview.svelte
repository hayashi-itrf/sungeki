<script lang="ts">
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { locale } from '../../shared/i18n/locale.svelte';
  import { onDestroy } from 'svelte';
  import AudioPreviewPlayer from './AudioPreviewPlayer.svelte';
  import { hostApi } from '../../shared/host-api';
  import type { EditorController } from '../editor/controller.svelte';
  import { AudioPreviewJobsController, previewActive, previewPhase } from './preview-jobs.svelte';
  let { editor }: { editor: EditorController } = $props();
  const jobs = (() => new AudioPreviewJobsController(hostApi, editor.snapshot!.session_id))();
  let open = $state(false), dialog = $state<HTMLDialogElement>();
  const stale = $derived(!!jobs.selected && (jobs.selected.stale || jobs.selected.source_revision !== editor.snapshot?.revision));
  const disabled = $derived(jobs.busy || !!jobs.pending || editor.busy || editor.drafting || !!editor.pending);
  $effect(() => { if (open && dialog) dialog.showModal(); });
  $effect(() => {
    if (!open || !jobs.selected || !previewActive(jobs.selected)) return;
    const timer = setInterval(() => { if (!jobs.busy && !jobs.pending) void jobs.refresh(); }, 1500);
    return () => clearInterval(timer);
  });
  onDestroy(() => jobs.dispose());
  async function show() { open = true; await jobs.list(); if (jobs.selected) await jobs.refresh(); }
  function close() { if (!jobs.busy && !jobs.pending) { open = false; } }
  async function generate() { await jobs.start(editor.snapshot!); }
</script>
<button disabled={editor.busy || editor.drafting || !!editor.pending} onclick={show}>{locale.text('audioPreviewListen')}</button>
{#if open && editor.snapshot}
  <dialog use:containDialogFocus bind:this={dialog} aria-label={locale.text('audioPreviewTitle')} oncancel={e => { e.preventDefault(); close(); }}>
    <header><h2>{locale.text('audioPreviewTitle')}</h2><button disabled={jobs.busy || !!jobs.pending} onclick={close}>{locale.text('close')}</button></header>
    <p>{locale.text('audioPreviewDescription')}</p>
    <button {disabled} onclick={generate}>{locale.text('audioPreviewGenerate')}</button>
    {#if jobs.pending}<p role="alert">{locale.text('audioPreviewPending')}</p><button disabled={jobs.busy} onclick={() => jobs.retry()}>{locale.text('audioPreviewRetry')}</button>{/if}
    {#if jobs.error}<p role="alert">{jobs.error}</p>{/if}
    {#if jobs.busy}<p role="status">{locale.text('audioPreviewChecking')}</p>{/if}
    <section aria-label={locale.text('audioPreviewHistory')}>
      <header><h3>{locale.text('audioPreviewHistory')}</h3><button disabled={jobs.busy || !!jobs.pending} onclick={() => jobs.list()}>{locale.text('audioPreviewRefreshList')}</button></header>
      <div class="history">{#each jobs.jobs as job (job.job_id)}
        <button disabled={jobs.busy || !!jobs.pending} onclick={() => { void jobs.refresh(job.job_id); }}>
          {locale.text('audioPreviewRevision')} {job.source_revision} — {previewPhase(job.phase)}（{job.job_id.slice(0, 8)}）
        </button>
      {:else}<p>{locale.text('audioPreviewEmpty')}</p>{/each}</div>
      {#if jobs.more}<button disabled={jobs.busy || !!jobs.pending} onclick={() => jobs.list(true)}>{locale.text('audioPreviewMore')}</button>{/if}
    </section>
    {#if jobs.selected}
      {@const job = jobs.selected}
      <section aria-label={locale.text('audioPreviewResult')}>
        <h3>{previewPhase(job.phase)}</h3>
        {#if job.progress && job.phase !== 'cancelled'}<p role="status">{job.progress.stage}</p>{/if}
        {#if job.error && job.phase !== 'cancelled'}<p role="alert">{job.error.message}</p>{/if}
        {#if stale}<p class="notice">{locale.text('audioPreviewStale')}</p>{/if}
        {#if job.output && job.phase === 'succeeded'}
          <p>{locale.text('audioPreviewSourceRevision')} {job.source_revision} ／ {(job.output.sample_frames / job.output.sample_rate).toFixed(2)} {locale.text('audioPreviewSeconds')}</p>
          {#if job.output.normalization}<p>{locale.text('audioPreviewMaster')} {job.output.normalization.target_lufs} LUFS ／ {locale.text('audioPreviewPeak')} {job.output.normalization.true_peak_db} dBTP（{locale.text('audioPreviewRemeasured')}）</p>{:else}<p>{locale.text('audioPreviewNormalizationOff')}</p>{/if}
          {#if !stale && !editor.drafting && !editor.pending && !editor.busy}
            {#key job.job_id}
              <AudioPreviewPlayer jobId={job.job_id} duration={job.output.sample_frames / job.output.sample_rate} />
            {/key}
          {/if}
          <p>{locale.text('audioPreviewLimits')}</p>
        {/if}
        <button disabled={jobs.busy || !!jobs.pending} onclick={() => jobs.refresh()}>{locale.text('audioPreviewRefresh')}</button>
        {#if previewActive(job)}<button disabled={jobs.busy || !!jobs.pending} onclick={() => jobs.cancel()}>{locale.text('audioPreviewCancel')}</button>{/if}
      </section>
    {/if}
    <p>{locale.text('audioPreviewCloseHint')}</p>
  </dialog>
{/if}
<style>
  header { display:flex; align-items:center; justify-content:space-between; gap:12px; }
  section { margin-top:16px; padding:14px; border:1px solid var(--panel-border); border-radius:6px; background:var(--background); }
  .history { display:flex; flex-direction:column; gap:6px; max-height:160px; overflow:auto; }
  .history button { text-align:left; }
  .notice { padding-left:10px; border-left:4px solid var(--accent); }
</style>
