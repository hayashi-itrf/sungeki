<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { save } from '@tauri-apps/plugin-dialog';
  import type { RenderJob } from '@sungeki/api-client';
  import type { PublicationController } from './publication.svelte';
  let { publication, job, disabled = false }: {
    publication: PublicationController; job: RenderJob; disabled?: boolean;
  } = $props();
  const result = $derived(publication.result?.job_id === job.job_id ? publication.result : null);
  function choose() {
    return publication.choose(job, () => save({ title: locale.text('videoSaveDialog'),
      defaultPath: `動画-${job.job_id.slice(0, 8)}.mp4`, filters: [{ name: 'MP4動画', extensions: ['mp4'] }] }));
  }
</script>
<div class="publish" aria-label={locale.text('videoSaveCopy')}>
  <button disabled={disabled || publication.busy || !!publication.pending} onclick={choose}>{locale.text('videoChooseSave')}</button>
  <p>{locale.text('videoCopyHint')}</p>
  {#if publication.busy}<p role="status">{locale.text('videoCopyBusy')}</p>{/if}
  {#if publication.error}<p role="alert">{publication.error}</p>{/if}
  {#if publication.pending && !publication.busy}
    <p role="alert">{locale.text('videoCopyUncertain')}</p>
    <label>{locale.text('subtitlePendingPath')}<input readonly value={publication.pending.path} onclick={e => e.currentTarget.select()} /></label>
    <button disabled={publication.busy} onclick={() => publication.retry()}>{locale.text('videoCopyRetry')}</button>
    <p>{locale.text('videoCopyRestart')}</p>
  {/if}
  {#if result}
    <p role="status">{result.already_present ? locale.text('videoCopyExisting') : locale.text('videoCopyDone')}</p>
    <label>{locale.text('videoSavedFile')}<input readonly value={result.output.path} onclick={e => e.currentTarget.select()} /></label>
    {#if result.stale}<p>{locale.text('videoCopyStale')}</p>{/if}
    <p>{locale.text('videoCopyVerified')}</p>
  {/if}
</div>
<style>
  .publish { margin-top:12px; padding:12px; border:1px solid var(--panel-border); border-radius:6px; background:var(--surface); }
  label { display:flex; flex-direction:column; gap:6px; }
  input { width:100%; }
</style>
