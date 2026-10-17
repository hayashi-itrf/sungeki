<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { renderPhase, type RenderJobsController } from './jobs.svelte';
  import RenderHistoryFailures from './RenderHistoryFailures.svelte';
  let { jobs, saving = false }: { jobs: RenderJobsController; saving?: boolean } = $props();
</script>
    <section aria-label={locale.text('renderHistory')}>
      <header><h3>{locale.text('renderHistory')}</h3><button disabled={jobs.busy || !!jobs.pending || saving} onclick={() => jobs.list()}>{locale.text('refreshList')}</button></header>
      <div class="history">{#each jobs.jobs as job (job.job_id)}<button class:selected={jobs.selected?.job_id === job.job_id}
        disabled={jobs.busy || !!jobs.pending || saving} onclick={() => jobs.refresh(job.job_id)}>{job.purpose === 'editing_preview' ? locale.text('renderPreview') : locale.text('renderFinal')} {locale.text('revision')} {job.source_revision} — {renderPhase[job.phase]}（{job.job_id.slice(0, 8)}）</button>{/each}</div>
      <RenderHistoryFailures {jobs} disabled={saving} />
      {#if !jobs.busy && !jobs.jobs.length && !jobs.unavailable.length}<p>{locale.text('renderEmpty')}</p>{/if}
      {#if jobs.more}<button disabled={jobs.busy || !!jobs.pending || saving} onclick={() => jobs.list(true)}>{locale.text('renderNext')}</button>{/if}
    </section>
<style>
  section { margin-top:18px; padding:14px; border:1px solid var(--panel-border); background:var(--background); border-radius:6px; }
  header { display:flex; justify-content:space-between; align-items:center; gap:12px; }
  h3 { margin:0; }
  .history { display:flex; flex-direction:column; gap:6px; margin:12px 0; max-height:180px; overflow:auto; }
  .history button { text-align:left; overflow-wrap:anywhere; }
  .selected { border:2px solid var(--accent); }
</style>
