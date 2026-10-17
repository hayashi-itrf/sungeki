<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { playbackVisibility } from './playback-visibility';
  let busy = $state(false), report = $state('');
  async function inspect() {
    busy = true;
    try { report = JSON.stringify(await playbackVisibility(), null, 2); }
    catch (error) { report = JSON.stringify({ error: String(error) }, null, 2); }
    finally { busy = false; }
  }
</script>
<button onclick={inspect} disabled={busy}>{locale.text('playbackInspectVisibility')}</button>
{#if report}<textarea aria-label={locale.text('playbackVisibilityResults')} readonly value={report} rows="6"></textarea>{/if}
<style>textarea { width:100%; }</style>
