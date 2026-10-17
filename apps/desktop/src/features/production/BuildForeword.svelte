<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { ForewordInput } from './foreword';
  let { foreword = $bindable() }: { foreword: ForewordInput } = $props();
</script>
<section aria-label={locale.text('productionForewordSettings')}>
  <label class="toggle"><input type="checkbox" bind:checked={foreword.enabled} />{locale.text('productionForewordEnable')}</label>
  {#if foreword.enabled}
    <p>{locale.text('productionForewordHint')}</p>
    <label>{locale.text('productionForewordText')}<textarea rows="4" required bind:value={foreword.text}></textarea></label>
    <label>{locale.text('productionForewordSeconds')}<input type="number" required min="1.6" max="60" step="0.1" bind:value={foreword.seconds} /></label>
  {/if}
</section>
<style>
  section { border:1px solid var(--panel-border); padding:12px; margin:12px 0; border-radius:6px; }
  label { display:grid; gap:6px; margin:8px 0; } .toggle { display:flex; align-items:center; }
  textarea,input[type=number] { width:100%; }
</style>
