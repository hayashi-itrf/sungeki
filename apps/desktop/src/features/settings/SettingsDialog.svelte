<script lang="ts">
  import GuideDialog from '../../shared/components/GuideDialog.svelte';
  import { locale } from '../../shared/i18n/locale.svelte';
  import AppearanceSettings from './AppearanceSettings.svelte';
  import RuntimeSettings from './RuntimeSettings.svelte';
  import SpeechConfiguration from './SpeechConfiguration.svelte';
  import PerformanceSettings from './PerformanceSettings.svelte';
  import ConnectionDiagnostics from './ConnectionDiagnostics.svelte';
  import BuildInfo from './BuildInfo.svelte';
  let { open = $bindable(false) }: { open?: boolean } = $props();
  let selected = $state('appearance');
  const items = $derived([
    { id:'appearance', label:locale.text('appearance') },
    { id:'runtime', label:locale.text('runtimeTitle') },
    { id:'speech', label:locale.current === 'ja' ? '音声エンジンの登録' : 'Voice engine registration' },
    { id:'performance', label:locale.text('perfTitle') },
    { id:'connection', label:locale.text('settingsConnection') },
    { id:'about', label:locale.current === 'ja' ? 'このアプリについて' : 'About this app' },
  ]);
</script>
<GuideDialog bind:open title={locale.text('settings')} {items} bind:selected>
  <!-- Keep drafts and in-flight actions when switching pages. -->
  <div hidden={selected !== 'appearance'}><AppearanceSettings /></div>
  <div hidden={selected !== 'runtime'}><RuntimeSettings /></div>
  <div hidden={selected !== 'speech'}><SpeechConfiguration /></div>
  <div hidden={selected !== 'performance'}><PerformanceSettings /></div>
  <div hidden={selected !== 'connection'}><ConnectionDiagnostics /></div>
  <div hidden={selected !== 'about'}><h2>{locale.current === 'ja' ? 'このアプリについて' : 'About this app'}</h2><BuildInfo /></div>
</GuideDialog>
