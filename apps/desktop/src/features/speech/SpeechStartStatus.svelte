<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  let { busy, retry }: { busy: boolean; retry: () => void } = $props();
  let elapsed = $state(0);
  $effect(() => {
    if (!busy) return;
    elapsed = 0;
    const started = performance.now();
    const timer = setInterval(() => { elapsed = Math.floor((performance.now() - started) / 1000); }, 1000);
    return () => clearInterval(timer);
  });
</script>
{#if busy}
  <p role="status">{locale.text('speechStartWaiting')}</p>
  <p>{locale.text('speechStartElapsed').replace('{seconds}', String(elapsed))}</p>
  <p>{locale.text('speechStartPreparationHint')}</p>
{:else}
  <p role="alert">{locale.text('speechStartUncertain')}</p>
  <button onclick={retry}>{locale.text('speechStartRetry')}</button>
{/if}
