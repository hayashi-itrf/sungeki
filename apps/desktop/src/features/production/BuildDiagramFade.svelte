<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { SceneFadeInput } from './scene-fade';
  let { fade = $bindable() }: { fade: SceneFadeInput | undefined } = $props();
</script>
<label><input type="checkbox" checked={!!fade} onchange={e => { fade = e.currentTarget.checked ? { inSeconds: 0.25, outSeconds: 0.25 } : undefined; }} />{locale.text('effectsDiagramFade')}</label>
{#if fade}
  <label>{locale.text('effectsIn')}<input type="number" required min="0" max="604800" step="any" bind:value={fade.inSeconds} /></label>
  <label>{locale.text('effectsOut')}<input type="number" required min="0" max="604800" step="any" bind:value={fade.outSeconds} /></label>
  <p>{locale.text('effectsDiagramFadeHint')}</p>
{/if}
<style>
  label { display:block; margin:10px 0; } input[type="number"] { display:block; width:100%; margin-top:6px; }
</style>
