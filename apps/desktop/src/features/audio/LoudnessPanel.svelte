<script lang="ts">
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { locale } from '../../shared/i18n/locale.svelte';
  import GainProposal from './GainProposal.svelte';
  import { audioLabel } from './label';
  import { onDestroy } from 'svelte';
  import { hostApi } from '../../shared/host-api';
  import type { EditorController } from '../editor/controller.svelte';
  import { LoudnessJobsController, loudnessActive, loudnessPhase } from './jobs.svelte';
  let { editor }: { editor: EditorController } = $props();
  const jobs = (() => new LoudnessJobsController(hostApi, editor.snapshot!.session_id))();
  let open = $state(false), dialog = $state<HTMLDialogElement>();
  const clip = $derived(editor.selection.length === 1 ? editor.snapshot?.project.clips.find(c => c.id === editor.selected) : undefined);
  const assetId = $derived(clip?.content.type === 'speech' ? clip.content.audio?.asset_id
    : clip?.content.type === 'audio' ? clip.content.asset_id : undefined);
  const source = $derived(editor.snapshot?.project.assets.find(a => a.id === assetId && a.probe.type === 'audio'));
  const busy = $derived(jobs.busy || !!jobs.pending);
  const blocked = $derived(busy || editor.busy || editor.drafting || !!editor.pending);
  $effect(() => { if (open && dialog) dialog.showModal(); });
  $effect(() => {
    if (!open || !jobs.selected || !loudnessActive(jobs.selected)) return;
    const timer = setInterval(() => { if (!jobs.busy && !jobs.pending) void jobs.refresh(); }, 1000);
    return () => clearInterval(timer);
  });
  onDestroy(() => jobs.dispose());
  async function show() { open = true; await jobs.list(); if (jobs.selected) await jobs.refresh(); }
  function close() { if (!jobs.busy && !jobs.pending) open = false; }
  function name(id: string) { return editor.snapshot ? audioLabel(editor.snapshot.project, id) : locale.text('loudnessMissing'); }
</script>
<button disabled={editor.busy || editor.drafting || !!editor.pending} onclick={show}>{locale.text('loudnessMeasure')}</button>
{#if open && editor.snapshot}
  <dialog use:containDialogFocus bind:this={dialog} aria-label={locale.text('loudnessTitle')} oncancel={e => { e.preventDefault(); close(); }}>
    <header><h2>{locale.text('loudnessTitle')}</h2><button disabled={busy} onclick={close}>{locale.text('close')}</button></header>
    <p>{locale.text('loudnessDescription')}</p>
    {#if source}<p>{locale.text('loudnessSource')}<strong>{name(source.id)}</strong></p>
    {:else}<p>{locale.text('loudnessSelect')}</p>{/if}
    <button disabled={blocked || !source} onclick={() => source && jobs.start(editor.snapshot!, source.id)}>{locale.text('loudnessStart')}</button>
    {#if jobs.pending}
      <p role="alert">{locale.text('loudnessPending')}</p>
      <button disabled={jobs.busy} onclick={() => jobs.retry()}>{locale.text('audioPreviewRetry')}</button>
    {/if}
    {#if jobs.error}<p role="alert">{jobs.error}</p>{/if}
    {#if jobs.busy}<p role="status">{locale.text('audioPreviewChecking')}</p>{/if}
    <section aria-label={locale.text('loudnessHistory')}>
      <header><h3>{locale.text('loudnessHistory')}</h3><button disabled={busy} onclick={() => jobs.list()}>{locale.text('audioPreviewRefreshList')}</button></header>
      <div class="history">{#each jobs.jobs as job (job.job_id)}
        <button class:selected={jobs.selected?.job_id === job.job_id} disabled={busy} onclick={() => jobs.refresh(job.job_id)}>
          {name(job.asset_id)} — {loudnessPhase(job.phase)}（{job.job_id.slice(0, 8)}）
        </button>
      {:else}<p>{locale.text('loudnessEmpty')}</p>{/each}</div>
      {#if jobs.more}<button disabled={busy} onclick={() => jobs.list(true)}>{locale.text('audioPreviewMore')}</button>{/if}
    </section>
    {#if jobs.selected}
      {@const job = jobs.selected}
      <section aria-label={locale.text('loudnessResult')}>
        <h3>{name(job.asset_id)}：{loudnessPhase(job.phase)}</h3>
        {#if job.stale || job.source_revision !== editor.snapshot.revision}<p class="notice">{locale.text('loudnessStale')}</p>{/if}
        {#if job.progress}<p role="status">{job.progress.stage}</p>{/if}
        {#if job.error}<p role="alert">{job.error.message}</p>{/if}
        {#if job.output}
          {@const result = job.output.measurement}
          {#if result.status === 'measured'}
            <dl><dt>{locale.text('loudnessIntegrated')}</dt><dd>{result.integrated_lufs.toFixed(1)}</dd>
              <dt>{locale.text('loudnessPeak')}</dt><dd>{result.true_peak_db.toFixed(1)}</dd>
              <dt>{locale.text('loudnessRange')}</dt><dd>{result.range_lu.toFixed(1)}</dd></dl>
          {:else}
            <p>{locale.text('loudnessUnmeasurable')}{result.reason === 'too_short' ? locale.text('loudnessShort') : locale.text('loudnessSilent')}</p>
            {#if result.true_peak_db !== null}<p>{locale.text('loudnessPeakValue')}{result.true_peak_db.toFixed(1)} dBTP</p>{/if}
          {/if}
          {#if result.status === 'measured' || result.true_peak_db !== null}
            <p>{locale.text('loudnessPrecision')}</p>
          {/if}
        {/if}
        <button disabled={busy} onclick={() => jobs.refresh()}>{locale.text('audioPreviewRefresh')}</button>
        {#if loudnessActive(job)}<button disabled={busy} onclick={() => jobs.cancel()}>{locale.text('loudnessCancel')}</button>{/if}
      </section>
    {/if}
    {#if jobs.selected?.phase === 'succeeded' && jobs.selected.output?.measurement.status === 'measured' && clip?.content.type === 'speech' && assetId === jobs.selected.asset_id}
      {#key `${jobs.selected.job_id}:${clip.id}`}
        <GainProposal {editor} job={jobs.selected} clipId={clip.id} disabled={busy} />
      {/key}
    {/if}
    <p>{locale.text('loudnessCloseHint')}</p>
  </dialog>
{/if}
<style>
  header { display:flex; justify-content:space-between; align-items:center; gap:12px; }
  header h3 { margin:0; }
  section { margin-top:18px; padding:14px; border:2px solid var(--panel-border); background:var(--background); border-radius:6px; }
  .history { display:flex; flex-direction:column; gap:6px; margin:12px 0; max-height:170px; overflow:auto; }
  .history button { text-align:left; overflow-wrap:anywhere; }
  .selected { border:2px solid var(--accent); }
  .notice { padding:10px; border-left:4px solid var(--accent); }
  dl { display:grid; grid-template-columns:1fr auto; gap:10px; }
  dd { margin:0; font-size:1.25rem; font-variant-numeric:tabular-nums; font-weight:700; }
</style>
