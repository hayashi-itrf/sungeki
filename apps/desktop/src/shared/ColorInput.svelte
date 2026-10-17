<script lang="ts">
  import { locale } from './i18n/locale.svelte';
  let { label, value = $bindable('#000000') }: { label: string; value: string } = $props();
</script>
<div class="color-field">
  <div class="label">{label}
    <span>
      <input class="swatch" type="color" aria-label={locale.current === 'en' ? `Choose ${label}` : `${label}を選ぶ`} bind:value />
      <input class="code" aria-label={locale.current === 'en' ? `${label} (hex)` : `${label}のカラーコード`} required pattern={'#[0-9A-Fa-f]{6}'} bind:value placeholder="#123456" />
    </span>
  </div>
</div>
<style>
  .label { display:flex; flex-direction:column; gap:4px; font-size:13px; margin-bottom:10px; }
  span { display:flex; gap:5px; align-items:center; }
  .swatch { flex:0 0 32px; width:32px; height:32px; padding:2px; cursor:pointer; }
  .code { min-width:0; width:100%; }
</style>
