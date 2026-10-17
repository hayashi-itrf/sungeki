<script lang="ts">
  import { collectionText as t } from './messages';
  import { onDestroy } from 'svelte';
  import { open as chooseDirectory } from '@tauri-apps/plugin-dialog';
  import Modal from '../../shared/components/Modal.svelte';
  import { hostApi } from '../../shared/host-api';
  import type { EditorController } from '../editor/controller.svelte';
  import { CollectionJobsController, collectionActive, collectionPhase } from './jobs.svelte';
  import { destination, canCollect, bytes, collectionError } from './presentation';
  import PlanItems from './PlanItems.svelte';
  import CollectionInterruption from './CollectionInterruption.svelte';
  let { editor, includeHistory = false, buttonLabel, checkpointNames = {} }: {
    editor: EditorController; includeHistory?: boolean; buttonLabel?: string; checkpointNames?: Record<string, string>;
  } = $props();
  const jobs = (() => new CollectionJobsController(hostApi, editor.snapshot!.session_id))();
  let opened = $state(false), choosing = $state(false), parent = $state(''), name = $state(t("素材付き作品"));
  let includeCheckpoints = $state(false);
  const target = $derived(destination(parent, name));
  const disabled = $derived(jobs.busy || !!jobs.pending || choosing || editor.busy || editor.drafting || !!editor.pending);
  const ready = $derived(canCollect(jobs.selected, editor.snapshot, target, includeCheckpoints));
  $effect(() => {
    if (!opened || !jobs.selected || !collectionActive(jobs.selected)) return;
    const timer = setInterval(() => { if (!jobs.busy && !jobs.pending) void jobs.refresh(); }, 1200);
    return () => clearInterval(timer);
  });
  onDestroy(() => jobs.dispose());
  async function show() {
    if (includeHistory) includeCheckpoints = true;
    opened = true; await jobs.list(); if (jobs.selected) await jobs.refresh();
  }
  async function inspect() {
    if (!target || !editor.snapshot) return;
    const requested = target;
    await jobs.start(editor.snapshot, requested, true, includeCheckpoints);
    adoptDestination(requested);
  }
  async function collect() {
    if (disabled || !ready || !target || !editor.snapshot) return;
    const requested = target, history = includeCheckpoints, reviewed = jobs.selected?.job_id;
    await jobs.refresh();
    if (jobs.error || target !== requested || history !== includeCheckpoints || jobs.selected?.job_id !== reviewed) return;
    if (!canCollect(jobs.selected, editor.snapshot, requested, history)) {
      jobs.error = t("作品または保存時点が変更されています。もう一度事前確認してください。");
      return;
    }
    await jobs.start(editor.snapshot!, requested, false, history);
  }
  async function retry() {
    const requested = jobs.pending?.destination;
    await jobs.retry();
    if (requested) adoptDestination(requested);
  }
  function adoptDestination(requested: string) {
    const job = jobs.selected;
    if (!jobs.pending && target === requested && job?.dry_run && job.source_revision === editor.snapshot?.revision) {
      parent = job.destination.slice(0, job.destination.lastIndexOf('/')) || '/';
    }
  }
  async function choose() {
    choosing = true;
    try { const path = await chooseDirectory({ title: t("素材付き作品を保存する場所"), directory: true, multiple: false }); if (path) parent = path; }
    catch (e) { jobs.error = String(e); }
    finally { choosing = false; }
  }
</script>
<button disabled={editor.busy || editor.drafting || !!editor.pending} onclick={show}>{buttonLabel ?? t("素材をまとめて保存")}</button>
<Modal bind:open={opened} title={t("素材をまとめて保存")}>
  {#if includeHistory}<p class="note">{t("復元前の素材確認には「名前付き保存時点も一緒に持ち出す」を選び、「コピーせず事前確認」を実行してください。現在の作品と全保存時点を確認します。事前確認では保存先フォルダの作成・素材コピー・編集状態の復元は行いません。")}</p>{/if}
  <p>{t("作品と登録素材を新しいフォルダへコピーします。元の作品は変更しません。")}</p>
  <p class="note">{t("名前付き保存時点は下の項目で選べます。Undo履歴は含みません。エンジン・モデル・フォントは別途必要です。素材の再配布可否は未確認です。")}</p>
  <section>
    <h3>{t("1. 保存先を選ぶ")}</h3>
    <button disabled={disabled} onclick={choose}>{t("保存する場所を選ぶ")}</button>
    <p>{parent || t("保存する場所を選んでください")}</p>
    <label>{t('新しいフォルダ名')} <input bind:value={name} disabled={disabled} /></label>
    {#if target}<p>{t("保存先：")}{target}</p>{:else if parent}<p role="alert">{t("フォルダ名に / や : は使えません。")}</p>{/if}
    <label><input type="checkbox" bind:checked={includeCheckpoints} disabled={disabled} /> {t('名前付き保存時点も一緒に持ち出す')}</label>
    {#if includeCheckpoints}<small>{t("全保存時点と過去の素材も対象です（最大127件）。過去素材が見つからない場合は保存できません。")}</small>{/if}
    <button disabled={disabled || !target || !editor.snapshot} onclick={inspect}>{t("2. コピーせず事前確認")}</button>
    <button disabled={disabled || !ready} onclick={collect}>{t("3. 素材付き作品を保存")}</button>
    <small>{t("保存先・作品・履歴の選択を変更した場合は、もう一度事前確認してください。保存時にも原本を再確認します。")}</small>
  </section>
  {#if jobs.error}<p role="alert">{jobs.error}</p>{/if}
  {#if jobs.pending}
    <p role="status">{t("開始結果を確認できません。同じ要求を再送して、二重実行を防ぎます。")}</p>
    <button disabled={jobs.busy} onclick={retry}>{t("開始結果を確認する")}</button>
  {/if}
  {#if jobs.selected}
    {@const job = jobs.selected}
    <section aria-label={t("収集結果")}>
      <h3>{job.dry_run ? t("事前確認") : t("素材の収集")}：{t(collectionPhase[job.phase])}</h3>
      <p>{job.include_checkpoints ? t('保存時点を含む（{count}件）', { count: job.checkpoint_count }) : t('現在の作品と登録素材のみ')}</p>
      <p>{t("対象の保存先：")}{job.destination}</p>
      {#if job.stale || job.source_revision !== editor.snapshot?.revision}<p class="note">{t("現在の作品とは異なる時点の結果です。")}</p>{/if}
      {#if job.error}<p role="alert">{collectionError(job.error)}</p>{/if}
      <CollectionInterruption {job} />
      {#if job.plan}
        <p>{job.plan.sources_ready ? t("必要な素材の供給元を確認しました。") : t("収集に必要な素材に問題があります。下の一覧を確認してください。")}</p>
        {#if job.plan.destination_exists}<p role="alert">{t("保存先が既に存在します。別のフォルダ名を指定してください。")}</p>{/if}
        <p>{t('保存時点 {count}件（通常の読み込みでは履歴を取り込みません）', { count: job.plan.checkpoint_count })}</p>
        <p>{t('素材 {count}件 → 重複を除いて {files}ファイル / {size}', { count: job.plan.source_count, files: job.plan.unique_files, size: bytes(job.plan.estimated_asset_bytes) })}</p>
        <small>{t("素材分だけの見積りです。作品データ等の追加容量が必要です。この確認結果は検査時点の記録です。")}</small>
        <PlanItems {jobs} {checkpointNames} />
      {/if}
      {#if job.output}<p>{t("保存できました：")}{job.output.directory}</p><p>{t('移動先で「作品・ファイル」から {path} を開いてください。', { path: job.output.project_path })}</p>{#if job.output.checkpoints_included}<p>{t("「同梱された保存時点も取り込む」を選ぶと履歴を利用できます。")}</p>{/if}{/if}
      <button disabled={jobs.busy || !!jobs.pending} onclick={() => jobs.refresh()}>{t("状態を更新")}</button>
      {#if collectionActive(job)}<button disabled={jobs.busy || !!jobs.pending || job.phase === 'cancelling'} onclick={() => jobs.cancel()}>{t("取り消す")}</button>{/if}
    </section>
  {/if}
  <section>
    <h3>{t("この作品の履歴")}</h3>
    <button disabled={jobs.busy || !!jobs.pending} onclick={() => jobs.list()}>{t("履歴を更新")}</button>
    {#each jobs.jobs as job (job.job_id)}
      <button class="history" disabled={jobs.busy || !!jobs.pending} onclick={() => jobs.refresh(job.job_id)}>{job.dry_run ? t("事前確認") : t("収集")}・{t(collectionPhase[job.phase])} — {job.destination}</button>
    {/each}
    {#if jobs.more}<button disabled={jobs.busy || !!jobs.pending} onclick={() => jobs.list(true)}>{t("続きを表示")}</button>{/if}
  </section>
</Modal>
<style>
  section { border:1px solid var(--border); border-radius:8px; padding:14px; margin:14px 0; background:var(--surface); }
  h3 { margin:0 0 10px; } p, small { overflow-wrap:anywhere; } small { display:block; color:var(--muted); margin:8px 0; }
  .note { padding:10px; border-left:3px solid var(--border); background:var(--workspace); }
  label { display:flex; gap:8px; align-items:center; margin:10px 0; } input:not([type="checkbox"]) { flex:1; min-width:0; }
  button { margin:4px 4px 4px 0; padding:8px 12px; } .history { display:block; width:100%; text-align:left; overflow-wrap:anywhere; }
</style>
