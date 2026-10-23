<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
</script>
<section>
  <h2>{locale.text('keyboardShortcuts')}</h2>
  <p>{locale.text('keyboardPlatform')}</p>
  <dl>
    <dt>{locale.text("playbackPlay")} / {locale.text("playbackPause")}</dt><dd>Space</dd>
    <dt>{locale.text('saveProject')}</dt><dd>⌘ / Ctrl + S</dd>
    <dt>{locale.text('saveAs')}</dt><dd>⌘ / Ctrl + Shift + S</dd>
    <dt>{locale.text('undo')}</dt><dd>⌘ / Ctrl + Z</dd>
    <dt>{locale.text('redo')}</dt><dd>{locale.text('redoAlternative')}</dd>
    <dt>{locale.text('deleteSelectedClips')}</dt><dd>Delete / Backspace</dd>
    <dt>{locale.text('frameStep')}</dt><dd>{locale.text('frameStepKeys')}</dd>
    <dt>{locale.text('frameBounds')}</dt><dd>Home / End</dd>
  </dl>
  <p>{locale.text('keyboardInputHint')}</p>
  <p>{locale.text('keyboardTimelineHint')}</p>
</section>
<style>
  dl { display:grid; grid-template-columns:1fr 1fr; gap:10px; }
  dt { font-weight:600; } dd { margin:0; }
</style>
