<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { ReactionInput } from './reaction';
  let { reaction = $bindable(), portrait = false }: { reaction: ReactionInput; portrait?: boolean } = $props();
</script>
<p>{locale.text('effectsReactionHint')}{portrait ? '1080×1920' : '1920×1080'}{locale.text('effectsPixelHint')}</p>
<div class="fields">
  <label>{locale.text('effectsCenterX')}<input required type="number" step="any" bind:value={reaction.x} /></label>
  <label>{locale.text('effectsCenterY')}<input required type="number" step="any" bind:value={reaction.y} /></label>
  <label>{locale.text('effectsWidth')}<input required type="number" min="1" step="any" bind:value={reaction.width} /></label>
  <label>{locale.text('effectsHeight')}<input required type="number" min="1" step="any" bind:value={reaction.height} /></label>
  <label>{locale.text('effectsStartScale')}<input required type="number" min="0.1" max="100" step="any" bind:value={reaction.scale} /></label>
  <label>{locale.text('effectsFadeIn')}<input required type="number" min="0" step="0.01" bind:value={reaction.fadeIn} /></label>
  <label>{locale.text('effectsFadeOut')}<input required type="number" min="0" step="0.01" bind:value={reaction.fadeOut} /></label>
</div>
<p>{locale.text('effectsReactionLimit')}</p>
<style>
  .fields { display:grid; grid-template-columns:repeat(auto-fit,minmax(130px,1fr)); gap:8px; }
  label { display:grid; gap:4px; } input { width:100%; min-width:0; }
</style>
