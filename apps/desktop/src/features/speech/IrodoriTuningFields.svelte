<script lang="ts">
  import { tuningText as t } from './tuning-messages';
  import type { IrodoriSettings } from '@sungeki/api-client';
  let { tuning = $bindable() }: { tuning: IrodoriSettings } = $props();
</script>
<label>{t("声の説明")}<textarea value={tuning.caption ?? ''} oninput={event => tuning.caption = event.currentTarget.value || null}></textarea></label>
<p>{t("空欄では登録音声の説明を使用します。説明への対応はモデルによって異なり、未対応の場合は合成開始時にお知らせします。")}</p>
<label>{t("条件への追従度（CFG）")}<input type="number" min="0" max="30" step="any" required bind:value={tuning.cfg_scale} /></label>
<label>{t("生成ステップ数")}<input type="number" min="1" max="200" step="1" required bind:value={tuning.num_steps} /></label>
<label>{t("生成シード")}<input type="number" min="0" max="4294967295" step="1" required bind:value={tuning.seed} /></label>
<p>{t("シードは生成時の乱数を指定する整数です。同じ条件での再生成に使用しますが、環境をまたぐ完全一致は保証しません。")}</p>
<style>label { display:block; margin:12px 0; } input,textarea { display:block; width:100%; }</style>
