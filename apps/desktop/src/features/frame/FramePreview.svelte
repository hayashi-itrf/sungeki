<script lang="ts">
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { frameText as t, frameStage } from './messages';
  import { onDestroy } from 'svelte';
  import { hostApi } from '../../shared/host-api';
  import type { EditorController } from '../editor/controller.svelte';
  import { FramePreviewJobsController, previewActive, previewPhase } from './jobs.svelte';
  import { FrameImage } from './image.svelte';
  import { PublicationController } from './publication.svelte';
  import PublishOutput from './PublishOutput.svelte';
  let { editor }: { editor: EditorController } = $props();
  const jobs = (() => new FramePreviewJobsController(hostApi, editor.snapshot!.session_id))();
  const publication = (() => new PublicationController(hostApi, editor.snapshot!.session_id))();
  const image = new FrameImage(hostApi);
  let open = $state(false), dialog = $state<HTMLDialogElement>(), frame = $state(0);
  const stale = $derived(!!jobs.selected && (jobs.selected.stale || jobs.selected.source_revision !== editor.snapshot?.revision));
  const saving = $derived(publication.busy || !!publication.pending);
  const disabled = $derived(jobs.busy || !!jobs.pending || saving || image.busy || editor.busy || editor.drafting || !!editor.pending);
  const validFrame = $derived(Number.isSafeInteger(frame) && frame >= 0);
  $effect(() => { if (open && dialog) dialog.showModal(); });
  $effect(() => {
    if (!open || !jobs.selected || !previewActive(jobs.selected)) return;
    const timer = setInterval(() => { if (!jobs.busy && !jobs.pending) void jobs.refresh(); }, 1500);
    return () => clearInterval(timer);
  });
  onDestroy(() => { jobs.dispose(); publication.dispose(); image.clear(); });
  async function show() {
    const fps = editor.snapshot!.project.canvas.fps;
    frame = Math.max(0, Math.floor(editor.time * fps.numerator / fps.denominator));
    open = true; await jobs.list(); if (jobs.selected) await jobs.refresh();
  }
  function close() { if (!jobs.busy && !jobs.pending && !publication.busy) { open = false; image.clear(); } }
  async function generate() { image.clear(); await jobs.start(editor.snapshot!, frame); }
</script>
<button disabled={editor.busy || editor.drafting || !!editor.pending} onclick={show}>{t("正式な画像を確認")}</button>
{#if open && editor.snapshot}
  <dialog use:containDialogFocus bind:this={dialog} aria-label={t("正式な画像を確認")} oncancel={e => { e.preventDefault(); close(); }}>
    <header><h2>{t("正式な画像を確認")}</h2><button disabled={jobs.busy || !!jobs.pending || publication.busy} onclick={close}>{t("閉じる")}</button></header>
    <p>{t("動画と同じ描画で1枚のPNGを作ります。字幕・立ち絵・配置の確認や画像の保存に使えます。音声の完成状態は検査しません。")}</p>
    <section aria-label={t("描画する位置")}>
      <label>{t("フレーム番号（先頭は0）")}<input type="number" min="0" step="1" bind:value={frame} {disabled} /></label>
      {#if validFrame}<p>{t('約 {seconds} 秒の画像', { seconds: (frame * editor.snapshot.project.canvas.fps.denominator / editor.snapshot.project.canvas.fps.numerator).toFixed(3) })}</p>{/if}
      <button disabled={disabled || !validFrame} onclick={generate}>{t("この位置の画像を作る")}</button>
      <p>{t("画面を開いた時の再生位置を初期値にしています。作品の終端以降は指定できません。")}</p>
    </section>
    {#if jobs.pending}<p role="alert">{t("開始結果が未確認です。")}</p><button disabled={jobs.busy} onclick={() => jobs.retry()}>{t("同じ要求を再確認")}</button>{/if}
    {#if jobs.error}<p role="alert">{jobs.error}</p>{/if}
    <section aria-label={t("確認画像の履歴")}>
      <header><h3>{t("確認画像の履歴")}</h3><button {disabled} onclick={() => jobs.list()}>{t("一覧を更新")}</button></header>
      <div class="history">{#each jobs.jobs as job (job.job_id)}
        <button {disabled} onclick={() => { image.clear(); void jobs.refresh(job.job_id); }}>{t('更新 {revision} — {phase}（{id}）', { revision: job.source_revision, phase: previewPhase(job.phase), id: job.job_id.slice(0, 8) })}</button>
      {:else}<p>{t("確認画像はまだありません。")}</p>{/each}</div>
      {#if jobs.more}<button {disabled} onclick={() => jobs.list(true)}>{t("次の12件")}</button>{/if}
    </section>
    {#if jobs.selected}
      {@const job = jobs.selected}
      <section aria-label={t("確認画像の結果")}>
        <h3>{previewPhase(job.phase)}</h3>
        {#if job.progress}<p role="status">{frameStage(job.progress.stage)}</p>{/if}
        {#if job.error}<p role="alert">{job.error.message}</p>{/if}
        {#if stale}<p class="notice">{t("作品または描画処理が変更されたため古い画像です。現在の内容が必要な場合は作り直してください。")}</p>{/if}
        {#if job.output && job.phase === 'succeeded'}
          <p>{t('更新 {revision} ／ フレーム {frame} ／ {seconds} 秒 ／ {width} × {height}', { revision: job.source_revision, frame: job.output.frame_index, seconds: (job.output.time_us / 1_000_000).toFixed(3), width: job.output.width, height: job.output.height })}</p>
          <button {disabled} onclick={() => image.load(job)}>{t("画像を読み込んで表示")}</button>
          {#if image.busy}<p role="status">{t("画像を検査しています…")}</p>{/if}
          {#if image.error}<p role="alert">{image.error}</p>{/if}
          {#if image.url && image.jobId === job.job_id}<img src={image.url} alt={t("正式描画による確認画像")} onerror={() => { image.clear(); image.error = t('画像を表示できません。再生成してください。'); }} />{/if}
          <PublishOutput {publication} {job} {disabled} />
        {/if}
        <button {disabled} onclick={() => jobs.refresh()}>{t("状態を更新")}</button>
        {#if previewActive(job)}<button disabled={jobs.busy || !!jobs.pending} onclick={() => jobs.cancel()}>{t("生成を取り消す")}</button>{/if}
      </section>
    {/if}
    <p>{t("画面を閉じても生成処理は続きます。元の素材の外部変更は、画像を作り直すと検査されます。")}</p>
  </dialog>
{/if}
<style>
  dialog { width:min(1000px, 94vw); }
  header { display:flex; align-items:center; justify-content:space-between; gap:12px; }
  section { margin-top:16px; padding:14px; border:1px solid var(--panel-border); border-radius:6px; background:var(--background); }
  .history { display:flex; flex-direction:column; gap:6px; max-height:160px; overflow:auto; }
  .history button { text-align:left; }
  img { display:block; width:100%; height:auto; margin-top:12px; border:1px solid var(--panel-border); }
  label { display:flex; gap:12px; align-items:center; flex-wrap:wrap; }
  .notice { padding:10px; border-left:4px solid var(--accent); background:var(--surface); }
</style>
