<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { onDestroy } from 'svelte';
  import Modal from '../../shared/components/Modal.svelte';
  import { hostApi } from '../../shared/host-api';
  import type { EditorController } from '../editor/controller.svelte';
  import { BuildJobsController, buildActive, buildPhase } from './jobs.svelte';
  import { observeBuildJob, shouldObserveBuild } from './observe';
  import { renderProgressLabel } from '../export/progress-label';
  import BuildResult from './BuildResult.svelte';
  import BuildFailure from './BuildFailure.svelte';
  import BuildAudioRecovery from './BuildAudioRecovery.svelte';
  import BuildForm from './BuildForm.svelte';
  import BuildTemplates from './BuildTemplates.svelte';
  import { TemplateLibrary } from './template-library.svelte';
  import { BrowserStartJournal } from './start-journal';
  import { BuildDrafts } from './drafts.svelte';
  import { voicevoxUrl } from '../speech/connection';
  let { editor }: { editor: EditorController } = $props();
  const drafts = new BuildDrafts(voicevoxUrl);
  const templates = new TemplateLibrary(hostApi);
  const jobs = new BuildJobsController(hostApi, new BrowserStartJournal());
  let open = $state(false);
  $effect(() => {
    if (!open || !shouldObserveBuild(jobs.selected)) return;
    return observeBuildJob(jobs);
  });
  onDestroy(() => { jobs.dispose(); templates.dispose(); });
  async function show() { open = true; await jobs.list(); if (jobs.selected) await jobs.refresh(); }
  async function edit(sessionId: string) {
    await editor.request({ type: 'get', session_id: sessionId });
    if (!editor.error) open = false;
  }
</script>
<button onclick={show}>{locale.text('buildTitle')}</button>
<Modal bind:open title={locale.text('buildTitle')} stickyHeader>
  {#if editor.snapshot}
    {#key editor.snapshot.session_id}
      {@const entry = drafts.entry(editor.snapshot.session_id)}
      <BuildForm {editor} {jobs} {templates} bind:draft={entry.value} />
    {/key}
    <p>{locale.text('buildDraftHint')}</p>
  {:else}
    <BuildTemplates library={templates} currentRecipe={null} {jobs} loadOnly
      disabled={jobs.busy || !!jobs.recoveryError || !!jobs.pending || editor.busy || editor.drafting || !!editor.pending} onbusy={() => {}} />
    <p>{locale.current === 'en' ? 'To create new settings or save a new template, open a project with speakers and media from Project / Files.' : '新しい設定を作る・テンプレートを新規保存する場合は、「作品・ファイル」から話者と素材を登録した作品を開いてください。'}</p>
  {/if}
  <p>{locale.text('buildHistoryHint')}</p>
  {#if jobs.recoveryError}<p role="alert">{jobs.recoveryError}</p><button onclick={() => jobs.restore()}>{locale.text('buildRestore')}</button>{/if}
  {#if jobs.pending}
    <p role="alert">{locale.text('buildPending')}</p>
    <p>{locale.text('buildPendingTitle')}{jobs.pending.recipe.title}</p>
    <p class="pending-path">{locale.text('buildPendingPath')}{jobs.pending.project_path}</p>
    <button disabled={jobs.busy} onclick={() => jobs.checkPending()}>{locale.text('buildCheckSaved')}</button>
    <button disabled={jobs.busy} onclick={() => jobs.retry()}>{locale.text('buildRetry')}</button>
  {/if}
  <button disabled={jobs.busy || !!jobs.pending} onclick={() => jobs.list()}>{locale.text('buildRefreshList')}</button>
  {#if jobs.error}<p role="alert">{jobs.error}</p>{/if}
  {#if jobs.busy}<p role="status">{locale.text('buildChecking')}</p>{/if}
  <div class="history">
    {#each jobs.jobs as job (job.job_id)}
      <button class:selected={job.job_id === jobs.selected?.job_id} disabled={jobs.busy || !!jobs.pending} onclick={() => jobs.refresh(job.job_id)}>{job.title} — {buildPhase[job.phase]}（{job.job_id.slice(0, 8)}）</button>
    {:else}{#if jobs.loaded && !jobs.unavailable.length}<p>{locale.text('buildEmpty')}</p>{/if}{/each}
  </div>
  {#each jobs.unavailable as entry (entry.id)}
    <p role="alert">{locale.text('buildUnavailable')}（{entry.id}）：{entry.message}</p>
    <button disabled={jobs.busy || !!jobs.pending} onclick={() => jobs.refresh(entry.id)}>{locale.text('buildRetryEntry')}（{entry.id.slice(0, 8)}）</button>
  {/each}
  {#if jobs.unavailable.length}<p>{locale.text('buildPartialList')}</p>{/if}
  {#if jobs.more}<button disabled={jobs.busy || !!jobs.pending} onclick={() => jobs.list(true)}>{locale.text('buildNext')}</button>{/if}
  {#if jobs.selected}
    {@const job = jobs.selected}
    <section aria-label={locale.text('buildSelected')}>
      <h3>{job.title}：{buildPhase[job.phase]}</h3>
      <p role="status">{job.progress ? renderProgressLabel(job.progress.stage, locale.current) : buildPhase[job.phase]}</p>
      <p>{locale.text('buildSteps')}{job.completed_steps} / {job.total_steps}{locale.text('buildStepsHint')}</p>
      {#if job.stale}<p class="notice">{locale.text('buildStale')}</p>{/if}
      <BuildFailure {job} />
      <BuildAudioRecovery {jobs} disabled={editor.busy || editor.drafting || !!editor.pending} />
      <button disabled={jobs.busy || !!jobs.pending} onclick={() => jobs.refresh()}>{locale.text('buildRefresh')}</button>
      {#if buildActive(job)}<button disabled={jobs.busy || (!!jobs.pending && jobs.pending.job_id !== job.job_id)} onclick={() => jobs.cancel()}>{locale.text('buildCancel')}</button>
        <p>{locale.text('buildCancelHint')}</p>
      {:else}<button disabled={jobs.busy || editor.busy || editor.drafting || !!editor.pending} onclick={() => edit(job.session_id)}>{locale.text('buildEdit')}</button>{/if}
      {#if editor.error}<p role="alert">{editor.error}</p>{/if}
      <BuildResult {job} />
    </section>
  {/if}
  <p>{locale.text('buildLastState')}</p>
</Modal>
<style>
  .pending-path { overflow-wrap:anywhere; user-select:text; }
  .history { display:grid; gap:6px; margin:12px 0; max-height:200px; overflow:auto; }
  .history button { text-align:left; overflow-wrap:anywhere; }
  .selected { border:2px solid var(--accent); }
  section { margin-top:16px; padding:14px; border:2px solid var(--panel-border); border-radius:6px; background:var(--background); }
  .notice { border-left:4px solid var(--accent); padding-left:10px; }
</style>
