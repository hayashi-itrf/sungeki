<script lang="ts">
  import type { CollectionJob } from '@sungeki/api-client';
  import { collectionText as t } from './messages';
  let { job }: { job: CollectionJob } = $props();
  const stage = $derived(job.error?.details.staging_path);
</script>

{#if job.phase === 'interrupted'}
  <aside aria-label={t('中断後の確認')}>
    <p>{t('この収集は完了していません。保存先を選び、「コピーせず事前確認」をやり直してから保存してください。')}</p>
    {#if stage}
      <p>{t('中断時の作業フォルダ（記録）：')}<span>{stage}</span></p>
      <p>{t('この場所は復旧時の記録です。現在の存在や内容は再確認していません。未完成の素材を完成品として使わないでください。')}</p>
    {/if}
  </aside>
{/if}

<style>
  aside { border-left:3px solid var(--border); background:var(--workspace); padding:10px; }
  p { margin:6px 0; overflow-wrap:anywhere; }
  span { display:block; user-select:text; }
</style>
