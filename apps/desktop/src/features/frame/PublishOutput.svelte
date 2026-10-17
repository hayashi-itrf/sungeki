<script lang="ts">
  import { frameText as t } from './messages';
  import { save } from '@tauri-apps/plugin-dialog';
  import type { FramePreviewJob } from '@sungeki/api-client';
  import type { PublicationController } from './publication.svelte';
  let { publication, job, disabled = false }: {
    publication: PublicationController; job: FramePreviewJob; disabled?: boolean;
  } = $props();
  const result = $derived(publication.result?.job_id === job.job_id ? publication.result : null);
  function choose() {
    return publication.choose(job, () => save({ title: t('完成画像の保存先'),
      defaultPath: t('画像-{id}.png', { id: job.job_id.slice(0, 8) }), filters: [{ name: t('PNG画像'), extensions: ['png'] }] }));
  }
</script>
<div class="publish" aria-label={t("完成画像を別の場所へ保存")}>
  <button disabled={disabled || publication.busy || !!publication.pending} onclick={choose}>{t("保存先を選んで保存")}</button>
  <p>{t("完成画像を確認してからコピーします。同名の別ファイルは上書きしません。")}</p>
  {#if publication.busy}<p role="status">{t("保存先の選択・画像の確認と保存中…")}</p>{/if}
  {#if publication.error}<p role="alert">{publication.error}</p>{/if}
  {#if publication.pending && !publication.busy}
    <p role="alert">{t("保存結果が未確認です。同じ保存先で再確認できます。")}</p>
    <label>{t("確認する保存先")}<input readonly value={publication.pending.path} onclick={e => e.currentTarget.select()} /></label>
    <button disabled={publication.busy} onclick={() => publication.retry()}>{t("同じ保存先で再確認")}</button>
    <p>{t("アプリを終了する場合は保存先を控えてください。再起動後に同じ出力と保存先を選ぶと、重複せず確認できます。")}</p>
  {/if}
  {#if result}
    <p role="status">{result.already_present ? t('同じ内容の画像が保存されていることを確認しました。') : t('指定した場所へ保存しました。')}</p>
    <label>{t("保存した画像")}<input readonly value={result.output.path} onclick={e => e.currentTarget.select()} /></label>
    {#if result.stale}<p>{t("保存開始時点で、作品または描画処理の変更前に作成された画像です。")}</p>{/if}
    <p>{t("保存時に完成原本との一致を確認済みです。画像内容の目視確認は別途必要です。")}</p>
  {/if}
</div>
<style>
  .publish { margin-top:12px; padding:12px; border:1px solid var(--panel-border); border-radius:6px; background:var(--surface); }
  label { display:flex; flex-direction:column; gap:6px; }
  input { width:100%; }
</style>
