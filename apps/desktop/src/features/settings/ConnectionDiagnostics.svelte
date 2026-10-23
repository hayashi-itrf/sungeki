<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import HostConnection from '../connection/HostConnection.svelte';
  import { checkNativeShell } from '../../shared/startup';
  let status = $state<string | null>(null), event = $state<string | null>(null), checking = $state(false);
  async function check() {
    checking = true; event = null;
    try { status = await checkNativeShell((message) => { event = message; }); }
    catch (error) { status = `${locale.text('startupFailed')}: ${String(error)}`; }
    finally { checking = false; }
  }
</script>
<h2>{locale.text('settingsConnection')}</h2>
<HostConnection />
<h3>{locale.text('startupTitle')}</h3>
<p>{locale.text('startupHint')}</p>
<button onclick={check} disabled={checking}>{checking ? locale.text('startupBusy') : locale.text('startupCheck')}</button>
<p role="status">{status ?? locale.text('startupIdle')}</p>
<p>{locale.text('startupEvent')}: <strong>{event ?? locale.text('startupNoEvent')}</strong></p>
