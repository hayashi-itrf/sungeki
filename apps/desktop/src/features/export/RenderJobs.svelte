<script lang="ts">
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { locale } from '../../shared/i18n/locale.svelte';
  import { renderProgressLabel } from './progress-label';
  import RenderHistory from './RenderHistory.svelte';
  import RenderError from './RenderError.svelte';
  import RenderPlayer from './RenderPlayer.svelte';
  import PublishOutput from './PublishOutput.svelte';
  import { PublicationController } from './publication.svelte';
  import { onDestroy } from 'svelte';
  import { hostApi } from '../../shared/host-api';
  import type { EditorController } from '../editor/controller.svelte';
  import { RenderJobsController, renderActive, renderPhase } from './jobs.svelte';
  let { editor }: { editor: EditorController } = $props();
  const jobs = (() => new RenderJobsController(hostApi, editor.snapshot!.session_id))();
  const publication = (() => new PublicationController(hostApi, editor.snapshot!.session_id))();
  const saving = $derived(publication.busy || !!publication.pending);
  let open = $state(false), dialog = $state<HTMLDialogElement>();
  const disabled = $derived(jobs.busy || !!jobs.pending || saving || editor.busy || editor.drafting || !!editor.pending);
  $effect(() => { if (open && dialog) dialog.showModal(); });
  $effect(() => {
    if (!open || !jobs.selected || !renderActive(jobs.selected)) return;
    const timer = setInterval(() => { if (!jobs.busy && !jobs.pending) void jobs.poll(); }, 1500);
    return () => clearInterval(timer);
  });
  onDestroy(() => { jobs.dispose(); publication.dispose(); });
  async function show() { open = true; await jobs.list(); if (jobs.selected) await jobs.refresh(); }
  function close() { if (!jobs.busy && !jobs.pending && !publication.busy) open = false; }
</script>
<button disabled={editor.busy || editor.drafting || !!editor.pending} onclick={show}>{locale.text('renderExport')}</button>
{#if open && editor.snapshot}
  <dialog use:containDialogFocus bind:this={dialog} aria-label={locale.text('renderTitle')} oncancel={e => { e.preventDefault(); close(); }}>
    <header><h2>{locale.text('renderTitle')}</h2><button disabled={jobs.busy || !!jobs.pending || publication.busy} onclick={close}>{locale.text('close')}</button></header>
    {#if jobs.selected && renderActive(jobs.selected)}
      <div class="active-progress">
        <p role="status"><strong>{renderPhase[jobs.selected.phase]}</strong><br />{jobs.selected.progress ? renderProgressLabel(jobs.selected.progress.stage, locale.current) : locale.text('renderChecking')}</p>
        <button disabled={jobs.busy || !!jobs.pending || saving || jobs.selected.phase === 'cancelling'} onclick={() => jobs.cancel()}>{locale.text(jobs.selected.phase === 'cancelling' ? 'renderCancelling' : 'renderCancel')}</button>
      </div>
    {/if}
    <p>{locale.text('renderIntro')}</p>
    <p>{locale.text('renderChapters')}</p>
    <p>{locale.text('renderStorage')}</p>
    <p>{locale.text('renderVideoAssets')}</p>
    {#if editor.snapshot.project.audio_mix.normalization}
      {@const target = editor.snapshot.project.audio_mix.normalization}
      <p>{locale.text('renderLevel')}{target.target_lufs} LUFS ／ {locale.text('renderPeak')}{target.true_peak_db} dBTP. {locale.text('renderLevelCheck')}</p>
    {:else}<p>{locale.text('renderLevelOff')}</p>{/if}
    <p>{editor.snapshot.project.canvas.width} × {editor.snapshot.project.canvas.height} ／ {editor.snapshot.project.canvas.fps.numerator / editor.snapshot.project.canvas.fps.denominator} fps</p>
    <button {disabled} onclick={() => jobs.start(editor.snapshot!)}>{locale.text('renderStart')}</button>
    {#if jobs.pending}<p role="alert">{locale.text('renderUncertain')}</p><button disabled={jobs.busy} onclick={() => jobs.retry()}>{locale.text('renderRetry')}</button>{/if}
    <RenderError message={jobs.error} actions={jobs.errorActions} />
    {#if jobs.busy}<p role="status">{locale.text('renderChecking')}</p>{/if}
    <RenderHistory {jobs} {saving} />
    {#if jobs.selected}
      {@const job = jobs.selected}
      <section aria-label={locale.text('renderSelected')}>
        <h3>{renderPhase[job.phase]}</h3>
        <p>{locale.text('renderStartRevision')} {job.source_revision}</p>
        {#if job.progress && !renderActive(job)}<p role="status">{locale.text('renderLastStage')} {renderProgressLabel(job.progress.stage, locale.current)}</p>{/if}
        {#if job.stale}<p class="notice">{locale.text('renderStale')}</p>{/if}
        <p>{locale.text('renderObserved')}</p>
        {#if job.error}<RenderError message={job.error.message} actions={job.error.suggested_actions} />{/if}
        {#if job.phase === 'succeeded' && job.output}
          <p>{locale.text('renderSaved')}{(job.output.byte_length / 1048576).toFixed(1)} MB ／ {job.output.frame_count} {locale.text('renderFrames')}</p>
          <label>{job.purpose === 'editing_preview' ? locale.text('renderPreviewFile') : locale.text('renderOriginal')}<input readonly value={job.output.path} onclick={event => event.currentTarget.select()} /></label>
          {#key job.job_id}<RenderPlayer {job} />{/key}
          {#if job.purpose === 'final'}<PublishOutput {publication} {job} disabled={jobs.busy || !!jobs.pending} />{:else}<p>{locale.text('renderPreviewWarning')}</p>{/if}
          <p>{locale.text('renderReviewWarning')}</p>
        {/if}
        <button disabled={jobs.busy || !!jobs.pending || saving} onclick={() => jobs.refresh()}>{locale.text('renderRefresh')}</button>
      </section>
    {/if}
    <p>{locale.text('renderCloseHint')}</p>
  </dialog>
{/if}
<style>
  header { display:flex; justify-content:space-between; align-items:center; gap:12px; }
  section { margin-top:18px; padding:14px; border:1px solid var(--panel-border); background:var(--background); border-radius:6px; }
  .active-progress { position:sticky; top:0; z-index:1; padding:12px; border:2px solid var(--accent); border-radius:6px; background:var(--surface); color:var(--text); }
  .active-progress { display:flex; justify-content:space-between; align-items:center; gap:12px; flex-wrap:wrap; }
  .active-progress p { margin:0; flex:1 1 240px; overflow-wrap:anywhere; }
  .notice { padding-left:10px; border-left:4px solid var(--accent); }
  label { display:flex; flex-direction:column; gap:6px; }
  input { width:100%; }
</style>
