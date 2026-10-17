<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { HostInfo } from '@sungeki/api-client';
  import { hostApi } from '../../shared/host-api';

  let host = $state<HostInfo | null>(null);
  let busy = $state(false);
  let errorMessage = $state('');
  const message = $derived(errorMessage || locale.text(host ? 'hostConnected' : 'hostDisconnected'));
  async function connect() {
    busy = true;
    host = null; errorMessage = '';
    try {
      host = await hostApi.getCapabilities();
    } catch (error) {
      errorMessage = typeof error === 'object' && error !== null && 'message' in error
        ? String(error.message) : String(error);
    } finally {
      busy = false;
    }
  }
</script>

<section aria-labelledby="host-title">
  <h2 id="host-title">{locale.text('hostTitle')}</h2>
  <p>{locale.text('hostHint')}</p>
  <button onclick={connect} disabled={busy}>{busy ? locale.text('hostBusy') : locale.text('hostConnect')}</button>
  <p role="status">{message}</p>
  {#if host}
    <p>{locale.text('hostCapabilityHint')}</p>
    <p>{locale.text('hostEditing')}: {host.capabilities.editing ? locale.text('hostAvailable') : locale.text('hostPreparing')} / {locale.text('hostSaving')}: {host.capabilities.persistence ? locale.text('hostAvailable') : locale.text('hostPreparing')} / {locale.text('hostRendering')}: {host.capabilities.rendering ? locale.text('hostAvailable') : locale.text('hostPreparing')}</p>
    <details><summary>{locale.text('hostDetails')}</summary><p>{locale.text('hostId')}: {host.host_instance_id}</p><p>{locale.text('hostSnapshotHint')}</p></details>
  {/if}
</section>
