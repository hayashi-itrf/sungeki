<script lang="ts">
  import RenderError from '../export/RenderError.svelte';
  import { locale } from '../../shared/i18n/locale.svelte';
  import { renderProgressLabel } from '../export/progress-label';
  import RenderHistoryFailures from '../export/RenderHistoryFailures.svelte';
  import { onDestroy, onMount } from 'svelte';
  import { hostApi } from '../../shared/host-api';
  import { RenderJobsController, renderActive, renderPhase } from '../export/jobs.svelte';
  import LivePreview from './LivePreview.svelte';
  import PlaybackVideo from './PlaybackVideo.svelte';
  import PlaybackProbe from './PlaybackProbe.svelte';
  import { playbackMatches, recoverPlayback } from './playback-clock';
  import type { EditorController } from './controller.svelte';
  let { editor, proxies, generation }: { editor: EditorController; proxies: Readonly<Record<string, string>>; generation: number } = $props();
  const jobs = (() => new RenderJobsController(hostApi, editor.snapshot!.session_id, 'editing_preview'))();
  let enabled = $state(false), reload = $state(0);
  let isolated = $state(false);
  let alive = true;
  const blocked = $derived(editor.busy || editor.drafting || !!editor.pending || !!editor.incoming || !!editor.dragPreview);
  const usable = $derived(playbackMatches(jobs.selected, editor.snapshot, blocked));
  $effect(() => {
    if (!jobs.selected || !renderActive(jobs.selected)) return;
    const timer = setInterval(() => { if (!jobs.busy && !jobs.pending) void jobs.poll(); }, 1500);
    return () => clearInterval(timer);
  });
  onMount(() => { void restore(false); });
  onDestroy(() => { alive = false; jobs.dispose(); });
  async function restore(show: boolean) {
    await jobs.list();
    if (!alive || jobs.error) return;
    const match = recoverPlayback(jobs.jobs, editor.snapshot);
    if (match) { jobs.selected = match; if (show) { reload++; enabled = true; } }
    else if (show && jobs.unavailable.length) jobs.error = locale.text('playbackUnavailable');
    else if (show) jobs.error = locale.text('playbackNoMatch');
  }
</script>
<div class="playback-options">
  {#if !enabled && usable}<button onclick={() => { enabled = true; }}>{locale.text('playbackShow')}</button>{/if}
  {#if enabled}<button onclick={() => { enabled = false; }}>{locale.text('playbackSimple')}</button>{/if}
<details>
  <summary>{locale.text('playbackManage')}</summary>
<div class="playback-actions">
  <button disabled={blocked || jobs.busy || !!jobs.pending || !!jobs.selected && renderActive(jobs.selected)} onclick={() => { enabled = true; void jobs.start(editor.snapshot!); }}>{usable ? locale.text('playbackRebuild') : locale.text('playbackPrepare')}</button>
  <button disabled={blocked || jobs.busy || !!jobs.pending || !!jobs.selected && renderActive(jobs.selected)} onclick={() => restore(true)}>{locale.text('playbackExisting')}</button>
  {#if enabled && usable}<button onclick={() => reload++}>{locale.text('playbackReload')}</button>{/if}
</div>
{#if jobs.selected}<small role="status">{locale.text('playbackSource')} {jobs.selected.purpose === 'final' ? locale.text('renderFinal') : locale.text('renderPreview')}</small>{/if}
  <p>{locale.text('playbackHint')}</p>
  <p>{locale.text('playbackSyncHint')}</p>
</details>
</div>
<RenderHistoryFailures {jobs} disabled={blocked} />
<RenderError message={jobs.error} actions={jobs.errorActions} />
{#if jobs.selected && ['failed', 'interrupted', 'cancelled'].includes(jobs.selected.phase)}
  <p role="status">{renderPhase[jobs.selected.phase]}{jobs.selected.error ? `：${jobs.selected.error.message}` : ''}</p>
{/if}
{#if jobs.pending}<button disabled={jobs.busy} onclick={() => jobs.retry()}>{locale.text('playbackRetry')}</button>{/if}
{#if jobs.selected && renderActive(jobs.selected)}
  <p role="status">{renderPhase[jobs.selected.phase]}{jobs.selected.progress ? `：${renderProgressLabel(jobs.selected.progress.stage, locale.current)}` : ''}</p>
  {#if jobs.selected.source_revision !== editor.snapshot?.revision}<small>{locale.text('playbackOldWork')}</small>{/if}
  <button disabled={jobs.busy || !!jobs.pending} onclick={() => jobs.cancel()}>{jobs.selected.purpose === 'final' ? locale.text('renderCancel') : locale.text('playbackCancel')}</button>
{/if}
{#if enabled && usable && jobs.selected}
  {#if import.meta.env.VITE_PLAYBACK_MEASUREMENT === '1'}
    <label><input type="checkbox" bind:checked={isolated} />{locale.text('playbackProbeMode')}</label>
  {/if}
  {#key `${jobs.selected.job_id}:${reload}:${isolated}`}
    {#if import.meta.env.VITE_PLAYBACK_MEASUREMENT === '1' && isolated}
      <PlaybackProbe jobId={jobs.selected.job_id} editorTime={() => editor.time} />
    {:else}<PlaybackVideo {editor} jobId={jobs.selected.job_id} />{/if}
  {/key}
{:else if editor.snapshot}
  {#if enabled && jobs.selected?.phase === 'succeeded' && !usable}<small>{locale.text('playbackStopped')}</small>{/if}
  {#key `${editor.snapshot.session_id}:${editor.snapshot.revision}`}
    <LivePreview {editor} {proxies} {generation} {blocked} />
  {/key}
{/if}

<style>.playback-actions { display:flex; flex-wrap:wrap; gap:6px; } button { font-size:12px; } small { color:var(--muted); } .playback-options { display:flex; align-items:flex-start; flex-wrap:wrap; gap:6px; flex-shrink:0; font-size:12px; } details { min-width:0; } summary { padding:6px; cursor:pointer; } details[open] { width:100%; } details p { margin:6px 0; }</style>
