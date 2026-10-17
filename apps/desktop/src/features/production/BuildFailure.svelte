<script lang="ts">
  import type { BuildVideoJob } from '@sungeki/api-client';
  import { locale } from '../../shared/i18n/locale.svelte';
  let { job }: { job: BuildVideoJob } = $props();
</script>
{#if job.error}
  <div class="failure" role="alert">
    <p>{job.error.message}</p>
    {#if job.error.suggested_actions.length}
      <p>{locale.text('buildRecoveryActions')}</p>
      <ul>{#each job.error.suggested_actions as action}<li>{action}</li>{/each}</ul>
    {/if}
    <details>
      <summary>{locale.text('buildFailureDetails')}</summary>
      <dl>
        <dt>{locale.text('buildErrorCode')}</dt><dd>{job.error.code}</dd>
        {#each Object.entries(job.error.details) as [key, value]}
          <dt>{key}</dt><dd>{value}</dd>
        {/each}
      </dl>
    </details>
  </div>
{/if}
{#if ['failed', 'interrupted', 'cancelled'].includes(job.phase)}
  <p>{locale.text('buildInterruptedHint')}</p>
{/if}
<style>
  .failure { border-left:4px solid var(--accent); padding:0 12px; overflow-wrap:anywhere; }
  li { margin:6px 0; }
  dd { margin:4px 0 12px; user-select:text; white-space:pre-wrap; }
</style>
