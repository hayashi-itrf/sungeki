<script lang="ts">
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { locale } from '../../shared/i18n/locale.svelte';
  import { proxyStage } from './proxy-labels';
  import { onDestroy, untrack } from 'svelte';
  import type { ProxySelection } from './proxy-selection.svelte';
  import type { ProxyPreset } from '@sungeki/api-client';
  import type { EditorController } from '../editor/controller.svelte';
  import { hostApi } from '../../shared/host-api';
  import { ProxyJobsController, proxyActive, proxyPhase } from './proxy-jobs.svelte';
  let { editor, selection }: { editor: EditorController; selection: ProxySelection } = $props();
  const jobs = untrack(() => new ProxyJobsController(hostApi, editor.snapshot!.session_id));
  onDestroy(() => jobs.dispose());
  let dialog: HTMLDialogElement;
  let opened = $state(false);
  let assetId = $state('');
  let preset = $state<ProxyPreset>('compact');
  const assets = $derived(editor.snapshot?.project.assets.filter(a => a.probe.type === 'video') ?? []);
  const blocked = $derived(jobs.busy || !!jobs.pending);
  const canStart = $derived(!blocked && !editor.busy && !editor.pending && !editor.drafting && !jobs.jobs.some(proxyActive));
  const name = (id: string) => assets.find(a => a.id === id)?.name ?? locale.text('proxyMissing');
  $effect(() => {
    if (!opened || !jobs.selected || !proxyActive(jobs.selected)) return;
    const timer = setInterval(() => { void jobs.refresh(); }, 1000);
    return () => clearInterval(timer);
  });
  function open() {
    const clip = editor.snapshot?.project.clips.find(c => c.id === editor.selected);
    assetId = clip?.content.type === 'video' ? clip.content.asset_id : assets[0]?.id ?? '';
    opened = true; dialog.showModal(); void jobs.list();
  }
  async function remove(asset: string) {
    if (await jobs.remove(asset)) selection.original(jobs.sessionId, asset);
  }
  async function start(all = false) {
    if (!editor.snapshot || !canStart) return;
    await jobs.start(editor.snapshot, all ? assets.map(a => a.id) : [assetId], preset);
  }
</script>
<button type="button" onclick={open}>{locale.text('proxyOpen')}</button>
<dialog use:containDialogFocus bind:this={dialog} onclose={() => opened = false} aria-label={locale.text('proxyTitle')}>
  <header><h2>{locale.text('proxyTitle')}</h2><button type="button" onclick={() => dialog.close()}>{locale.text('close')}</button></header>
  <p>{locale.text('proxyDescription')}</p>
  <p class="notice">{locale.text('proxyHint')}</p>
  <section aria-label={locale.text('proxySource')}>
    <h3>{locale.text('proxyDisplay')}</h3>
    {#each assets as asset (asset.id)}
      <p>{asset.name}：{selection.forSession(editor.snapshot!.session_id)[asset.id] ? locale.text('proxyChosen') : locale.text('proxyOriginal')}
        {#if selection.forSession(editor.snapshot!.session_id)[asset.id]}<button type="button" onclick={() => selection.original(editor.snapshot!.session_id, asset.id)}>{locale.text('proxyRestore')}</button>{/if}</p>
    {/each}
  </section>
  <section aria-label={locale.text('proxyTargets')}>
    <label>{locale.text('proxyAsset')}<select bind:value={assetId} disabled={blocked || !assets.length}>
      {#each assets as asset (asset.id)}<option value={asset.id}>{asset.name}</option>{/each}
    </select></label>
    <label>{locale.text('proxySize')}<select bind:value={preset} disabled={blocked}>
      <option value="compact">{locale.text('proxyCompact')}</option><option value="standard">{locale.text('proxyStandard')}</option>
    </select></label>
    <div class="actions"><button type="button" disabled={!canStart || !assetId} onclick={() => start()}>{locale.text('proxyGenerate')}</button>
      <button type="button" disabled={!canStart || !assets.length || assets.length > 64} onclick={() => start(true)}>{locale.text('proxyGenerateAll')}</button></div>
    {#if !assets.length}<p>{locale.text('proxyNoAssets')}</p>{/if}
    {#if editor.drafting || editor.pending}<p>{locale.text('proxyDraft')}</p>{/if}
    {#if jobs.pending}<p role="alert">{locale.text('proxyPending')}</p>
      <button type="button" disabled={jobs.busy} onclick={() => jobs.retry()}>{locale.text('proxyRetry')}</button>{/if}
    {#if jobs.notice}<p role="status">{jobs.notice}</p>{/if}
    {#if jobs.error}<p role="alert">{jobs.error}</p>{/if}
  </section>
  <section aria-label={locale.text('proxyHistoryLabel')}>
    <header><h3>{locale.text('proxyHistory')}</h3><button type="button" disabled={blocked} onclick={() => jobs.list()}>{locale.text('audioPreviewRefreshList')}</button></header>
    <div class="history">{#each jobs.jobs as job (job.job_id)}
      <button type="button" class:selected={jobs.selected?.job_id === job.job_id} disabled={blocked} onclick={() => jobs.refresh(job.job_id)}>
        {job.asset_ids.length === 1 ? name(job.asset_ids[0]) : `${job.asset_ids.length} ${locale.text('proxyVideos')}`} — {proxyPhase(job.phase)}
      </button>
    {:else}<p>{locale.text('proxyEmpty')}</p>{/each}</div>
    {#if jobs.more}<button type="button" disabled={blocked} onclick={() => jobs.list(true)}>{locale.text('proxyMore')}</button>{/if}
  </section>
  {#if jobs.selected}
    {@const job = jobs.selected}
    <section aria-label={locale.text('proxyStatus')}>
      <h3>{proxyPhase(job.phase)}：{job.completed.length} / {job.asset_ids.length} {locale.text('proxyCount')}</h3>
      {#if job.progress}<p role="status">{proxyStage(job.progress.stage)}</p>{/if}
      {#if job.stale || job.source_revision !== editor.snapshot?.revision}<p class="notice">{locale.text('proxyStale')}</p>{/if}
      {#if job.error}<p role="alert">{job.error.message}</p>{/if}
      {#if job.completed.length}<ul>{#each job.completed as item (item.asset_id)}<li>{name(item.asset_id)}: {locale.text('proxyVerified')} · {(item.byte_length / 1024 / 1024).toFixed(1)}MiB
        <button type="button" disabled={!assets.some(a => a.id === item.asset_id)} onclick={() => { if (editor.snapshot) selection.use(editor.snapshot, job, item.asset_id); }}>{locale.text('proxyUse')}</button></li>{/each}</ul>{/if}
      {#if !proxyActive(job)}
        <p>{locale.text('proxyRemoveHint')}</p>
        {#each job.asset_ids as asset (asset)}<button type="button" disabled={blocked} onclick={() => remove(asset)}>{locale.text('proxyRemove')}: {name(asset)}</button>{/each}
      {/if}
      <button type="button" disabled={blocked} onclick={() => jobs.refresh()}>{locale.text('audioPreviewRefresh')}</button>
      {#if proxyActive(job)}<button type="button" disabled={blocked || job.phase === 'cancelling'} onclick={() => jobs.cancel()}>{locale.text('proxyCancel')}</button>{/if}
    </section>
  {/if}
  <p>{locale.text('proxyCloseHint')}</p>
</dialog>
<style>
  header { display:flex; align-items:center; justify-content:space-between; gap:12px; } h3 { margin:0; }
  section { margin:14px 0; padding:14px; border:2px solid var(--panel-border); border-radius:6px; background:var(--background); }
  label { display:flex; flex-direction:column; gap:6px; margin:10px 0; }
  .actions { display:flex; gap:6px; flex-wrap:wrap; }
  .history { display:flex; flex-direction:column; gap:6px; max-height:180px; overflow:auto; margin-top:10px; }
  .history button { text-align:left; overflow-wrap:anywhere; } .selected { border:2px solid var(--accent); }
  .notice { border-left:4px solid var(--accent); padding:10px; background:var(--panel-header); } p { line-height:1.6; }
</style>
