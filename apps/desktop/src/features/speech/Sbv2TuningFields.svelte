<script lang="ts">
  import { tuningText as t } from './tuning-messages';
  import { tuningFields, type Sbv2Tuning } from './sbv2-tuning';
  let { tuning = $bindable() }: { tuning: Sbv2Tuning } = $props();
</script>
{#each tuningFields as field}
  <label>{t(field.label)}<input type="number" min={field.min} max={field.max} step="any" required bind:value={tuning[field.key]} /></label>
{/each}
<label><input type="checkbox" bind:checked={tuning.auto_split} /> {t("文ごとに分割して合成する")}</label>
<p>{t("文の間隔は分割を有効にした場合に使います。話す長さの倍率は1が標準です。設定の効果はモデルによって異なるため、再生成後に確認再生してください。")}</p>
<style>label { display:block; margin:12px 0; } input[type=number] { display:block; width:100%; } p { font-size:12px; }</style>
