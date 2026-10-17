<script lang="ts">
  import BuildInfo from '../features/settings/BuildInfo.svelte';
  import PerformanceSettings from '../features/settings/PerformanceSettings.svelte';
  import { locale } from '../shared/i18n/locale.svelte';
  import EditorWorkspace from '../features/editor/EditorWorkspace.svelte';
  import HostConnection from '../features/connection/HostConnection.svelte';
  import SpeechConfiguration from '../features/settings/SpeechConfiguration.svelte';
  import RuntimeSettings from '../features/settings/RuntimeSettings.svelte';
  import AppearanceSettings from '../features/settings/AppearanceSettings.svelte';
  import KeyboardShortcuts from '../features/settings/KeyboardShortcuts.svelte';
  import { checkNativeShell } from '../shared/startup';
  import { containDialogFocus } from '../shared/components/dialog-focus';

  let settings: HTMLDialogElement;
  let status = $state<string | null>(null);
  let event = $state<string | null>(null);
  let checking = $state(false);

  async function check() {
    checking = true;
    event = null;
    try {
      status = await checkNativeShell((message) => { event = message; });
    } catch (error) {
      status = `${locale.text('startupFailed')}: ${String(error)}`;
    } finally {
      checking = false;
    }
  }
</script>

<main>
  <EditorWorkspace onopensettings={() => settings.showModal()} />
  <dialog bind:this={settings} use:containDialogFocus aria-label={locale.text('settingsDiagnostics')}>
  <div class="settings-heading"><h2>{locale.text('settings')}</h2><button onclick={() => settings.close()}>{locale.text('close')}</button></div>
  <BuildInfo />
  <AppearanceSettings />
  <RuntimeSettings />
  <SpeechConfiguration />
  <KeyboardShortcuts />
  <PerformanceSettings />
  <details><summary>{locale.text('settingsConnection')}</summary>
  <HostConnection />
  <section aria-labelledby="startup-title">
    <h2 id="startup-title">{locale.text('startupTitle')}</h2>
    <p>{locale.text('startupHint')}</p>
    <button onclick={check} disabled={checking}>{checking ? locale.text('startupBusy') : locale.text('startupCheck')}</button>
    <p role="status">{status ?? locale.text('startupIdle')}</p>
    <p>{locale.text('startupEvent')}: <strong>{event ?? locale.text('startupNoEvent')}</strong></p>
  </section>
  </details>
  <aside>
    <h2>{locale.text('settingsScope')}</h2>
    <p>{locale.text('scopeEditing')}</p>
    <p>{locale.text('scopeAudio')}</p>
    <p>{locale.text('scopePreview')}</p>
  </aside>
</dialog>
</main>
<style>
.settings-heading { display:flex; align-items:center; justify-content:space-between; gap:12px; }
</style>
