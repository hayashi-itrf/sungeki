<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { RenderJobsController } from './jobs.svelte';
  let { jobs, disabled = false }: { jobs: RenderJobsController; disabled?: boolean } = $props();
</script>
{#each jobs.unavailable as entry (entry.id)}
  <div class="failure" role="alert">
    <p>{locale.text('renderReadFailed')}（{entry.id.slice(0, 8)}）：{entry.message}</p>
    <button disabled={disabled || jobs.busy || !!jobs.pending} onclick={() => jobs.refresh(entry.id)}>{locale.text('renderReadRetry')}</button>
  </div>
{/each}
{#if jobs.selected && jobs.unavailable.some(entry => entry.id === jobs.selected?.job_id)}
  <p role="alert">{locale.text('renderLastObservation')}</p>
{/if}
<style>.failure { padding:8px; border:1px solid var(--panel-border); overflow-wrap:anywhere; } p { margin:4px 0; }</style>
