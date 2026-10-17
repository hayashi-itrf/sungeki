<script lang="ts">
  import { tuningText as t } from './tuning-messages';
  import type { Character } from '@sungeki/api-client';
  type Settings = Extract<Character['voice'], { type: 'qwen' }>['settings'];
  let { tuning = $bindable() }: { tuning: Settings } = $props();
</script>
<label>{t("話速（1が標準）")}<input type="number" min="0.25" max="4" step="any" required bind:value={tuning.speed} /></label>
<label>{t("話し方の指示")}<textarea value={tuning.instruct ?? ''} oninput={event => tuning.instruct = event.currentTarget.value || null}></textarea></label>
<p>{t("指示の効果は使用するモデルによって異なります。")}</p>
<style>label { display:block; margin:12px 0; } input,textarea { display:block; width:100%; }</style>
