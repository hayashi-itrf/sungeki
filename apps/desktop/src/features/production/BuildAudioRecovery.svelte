<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { recoverySpeechStep, type BuildJobsController } from './jobs.svelte';
  let { jobs, disabled = false }: { jobs: BuildJobsController; disabled?: boolean } = $props();
  const job = $derived(jobs.selected);
  const outcome = $derived(jobs.audioRecovery?.jobId === job?.job_id ? jobs.audioRecovery : null);
</script>
{#if job?.phase === 'interrupted' && (recoverySpeechStep(job) || outcome)}
  <aside aria-label={locale.text('buildAudioRecoveryTitle')}>
    <h4>{locale.text('buildAudioRecoveryTitle')}</h4>
    {#if outcome?.applied}
      <p role="status">{locale.text('buildAudioRecoveryDone')}</p>
    {:else}
      <p>{locale.text('buildAudioRecoveryHint')}</p>
      <p>{locale.text('buildAudioRecoveryScope')}</p>
      {#if outcome && !jobs.busy}<p role="status">{locale.text('buildAudioRecoveryPending')}</p>{/if}
      {#if job.stale}<p>{locale.text('buildStale')}</p>{/if}
      <button disabled={disabled || jobs.busy || !!jobs.pending || job.stale} onclick={() => jobs.reconcileAudio()}>
        {locale.text(outcome ? 'buildAudioRecoveryRetry' : 'buildAudioRecoveryAction')}
      </button>
    {/if}
  </aside>
{/if}
<style>
  aside { margin:14px 0; padding:14px; border:2px solid var(--panel-border); border-radius:8px; background:var(--surface); }
  h4 { margin:0 0 8px; }
  p { margin:8px 0; }
</style>
