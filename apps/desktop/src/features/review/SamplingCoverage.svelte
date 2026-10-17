<script lang="ts">
  import type { ReviewFrameSampling } from '@sungeki/api-client';
  import { coverageEnglish, coverageJapanese } from '../../shared/i18n/sampling-coverage';
  import { locale } from '../../shared/i18n/locale.svelte';
  let { sampling }: { sampling: ReviewFrameSampling } = $props();
  let limit = $state(20);
  const text = $derived(locale.current === 'en' ? coverageEnglish : coverageJapanese);
</script>
{#if sampling.omitted.length || sampling.unsampled.length}
  <details>
    <summary>{text.title}</summary>
    <p>{text.caution}</p>
    {#if sampling.omitted.length}
      <h5>{text.omitted}</h5>
      <ul>{#each sampling.omitted.slice(0, limit) as sample}<li>{(sample.time_us / 1_000_000).toFixed(6)}</li>{/each}</ul>
    {/if}
    {#if sampling.unsampled.length}
      <h5>{text.spans}</h5><p>{text.hint}</p>
      <ul>{#each sampling.unsampled.slice(0, limit) as span}<li>{span.first} – {span.end - 1}</li>{/each}</ul>
    {/if}
    {#if sampling.omitted.length > limit || sampling.unsampled.length > limit}
      <button onclick={() => limit += 20}>{text.more}</button>
    {/if}
  </details>
{/if}
<style>
  details { margin-top:10px; }
  h5 { margin:12px 0 6px; }
  ul { max-height:200px; overflow:auto; margin:6px 0; }
</style>
