<script lang="ts">
  import EditorWorkspace from '../features/editor/EditorWorkspace.svelte';
  import SettingsDialog from '../features/settings/SettingsDialog.svelte';
  import HelpDialog from '../features/help/HelpDialog.svelte';
  let settingsOpen = $state(false);
  let helpOpen = $state(false);
</script>
<main>
  <EditorWorkspace onopensettings={() => settingsOpen = true} onopenhelp={() => helpOpen = true} />
  <SettingsDialog bind:open={settingsOpen} />
  <HelpDialog bind:open={helpOpen} />
</main>
