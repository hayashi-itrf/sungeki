<script lang="ts">
  import { tuningText as t } from './tuning-messages';
  import { voicevoxTuningFields, type VoicevoxTuning } from './voicevox-tuning';
  let { tuning = $bindable() }: { tuning: VoicevoxTuning } = $props();
</script>
{#each voicevoxTuningFields as field}
  <label>{t(field.label)}<input type="number" min={field.min} step="any" required bind:value={tuning[field.key]} /></label>
{/each}
<label><input type="checkbox" checked={tuning.pause_length !== null} onchange={event => { tuning.pause_length = event.currentTarget.checked ? .1 : null; }} />{t("句読点の間を秒数で指定する")}</label>
{#if tuning.pause_length !== null}<label>{t("句読点の間（秒）")}<input type="number" min="0" step="any" required bind:value={tuning.pause_length} /></label>{/if}
<label><input type="checkbox" bind:checked={tuning.enable_interrogative_upspeak} /> {t("疑問文の語尾を上げる")}</label>
<p>{t("話速は0より大きい値が必要です。句読点の秒数を指定しない場合はエンジンの値を使います。音量だけを変えたい場合は、セリフの音量調整なら再合成せず変更できます。")}</p>
<style>label { display:block; margin:12px 0; } input[type=number] { display:block; width:100%; } p { font-size:12px; }</style>
