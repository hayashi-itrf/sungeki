<script lang="ts">
  import { collectionText as t } from './messages';
  import type { CollectionJobsController } from './jobs.svelte';
  import { sourceStatus, bytes } from './presentation';
  let { jobs, checkpointNames = {} }: { jobs: CollectionJobsController; checkpointNames?: Record<string, string> } = $props();
</script>
{#if jobs.selected?.plan}
  <div class="items">
    <h3>{t("素材ごとの確認")}</h3>
    <button disabled={jobs.busy || !!jobs.pending} onclick={() => jobs.loadPage()}>{t("素材一覧を表示")}</button>
    {#if jobs.page}
      <p>{jobs.page.total === 0 ? t("登録素材はありません。") : t('{first}〜{last}件 / {total}件', { first: jobs.page.offset + 1, last: jobs.page.offset + jobs.page.items.length, total: jobs.page.total })}</p>
      {#each jobs.page.items as item (`${item.checkpoint_id ?? "current"}:${item.asset_id}`)}
        <article>
          <strong>{t(sourceStatus[item.status])}{item.conflicting_metadata ? t("・登録情報の矛盾あり") : ''}</strong>
          <span>{bytes(item.byte_length)}</span>
          <p>{item.checkpoint_id ? t('保存時点: {name}', { name: checkpointNames[item.checkpoint_id] ?? item.checkpoint_id }) : t('現在の作品')}</p>
          {#if item.checkpoint_id && checkpointNames[item.checkpoint_id]}<small>{t("識別：")}{item.checkpoint_id}</small>{/if}
          <p>{item.source_path ?? t("参照先を解決できません")}{item.source_path_truncated ? t("（長いパスの後半を省略）") : ''}</p>
          {#if item.detail}<p>{item.detail}</p>{/if}
        </article>
      {/each}
      <div class="paging">
        <button disabled={jobs.busy || !!jobs.pending || jobs.page.offset === 0} onclick={() => jobs.loadPage(Math.max(0, jobs.page!.offset - 25))}>{t("前の25件")}</button>
        <button disabled={jobs.busy || !!jobs.pending || jobs.page.offset + jobs.page.items.length >= jobs.page.total} onclick={() => jobs.loadPage(jobs.page!.offset + jobs.page!.items.length)}>{t("次の25件")}</button>
      </div>
    {/if}
  </div>
{/if}
<style>
  .items { border-top:1px solid var(--border); margin-top:12px; padding-top:12px; }
  article { border:1px solid var(--border); border-radius:6px; padding:10px; margin:8px 0; background:var(--surface); }
  article span { float:right; } p { overflow-wrap:anywhere; margin:6px 0; }
  .paging { display:flex; gap:8px; }
</style>
