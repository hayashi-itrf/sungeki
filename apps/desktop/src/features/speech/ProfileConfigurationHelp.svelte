<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  let { path }: { path: string | null } = $props();
</script>
{#if path}
  <p role="status">{locale.text('voiceConfigurationMissing')}</p>
  <details>
    <summary>{locale.text('voiceConfigurationLocation')}</summary>
    <code>{path}</code>
    <p>{locale.text('voiceConfigurationSetup')}</p>
  </details>
{/if}
<style>code { overflow-wrap:anywhere; } details { margin:10px 0; }</style>
