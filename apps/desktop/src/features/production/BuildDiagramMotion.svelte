<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { DiagramMotionInput } from './diagram-motion';
  let { motion = $bindable() }: { motion: DiagramMotionInput } = $props();
</script>
<p>{locale.text('effectsMotionHint')}</p>
<label>{locale.text('effectsEndX')}<input type="number" required min="-1000000" max="1000000" step="any" bind:value={motion.endX} /></label>
<label>{locale.text('effectsEndY')}<input type="number" required min="-1000000" max="1000000" step="any" bind:value={motion.endY} /></label>
<label>{locale.text('effectsEndScale')}<input type="number" required min="0.000001" step="any" bind:value={motion.scale} /></label>
<label>{locale.text('effectsEasing')}<select bind:value={motion.easing}><option value="ease_in_out">{locale.text('effectsSmooth')}</option><option value="linear">{locale.text('effectsLinear')}</option></select></label>
<style>label { display:grid; gap:6px; margin:8px 0; } input,select { width:100%; }</style>
