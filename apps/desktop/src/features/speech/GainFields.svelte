<script lang="ts">
  import { uiText } from '../../shared/i18n/extended-ui';
  let { enabled = $bindable(false), gainDb = $bindable(0) }: { enabled?: boolean; gainDb?: number } = $props();
</script>
<div class="gain">
  <label class="toggle"><input type="checkbox" bind:checked={enabled} />{uiText("発話の音量を調整")}</label>
  {#if enabled}
    <label>{uiText("音量（dB）")}<input type="number" min="-60" max="24" step="any" required bind:value={gainDb} /></label>
  {/if}
  <p>{uiText("0 dBは元の音量です。変更は動画出力に反映され、元音声と波形は変わりません。増幅すると音割れする場合があります。")}</p>
</div>
<style>
  .gain { border:1px solid var(--panel-border); border-radius:5px; padding:8px; margin-bottom:12px; }
  label { display:flex; flex-direction:column; gap:4px; font-size:13px; }
  .toggle { flex-direction:row; align-items:center; margin-bottom:8px; }
  p { font-size:11px; color:var(--muted); margin-bottom:0; }
</style>
