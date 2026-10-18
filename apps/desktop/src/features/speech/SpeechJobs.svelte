<script lang="ts">
  import LocalConnections from "./LocalConnections.svelte";
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { locale } from '../../shared/i18n/locale.svelte';
  import { onDestroy, tick } from 'svelte';
  import type { Clip } from '@sungeki/api-client';
  import { hostApi } from '../../shared/host-api';
  import type { EditorController } from '../editor/controller.svelte';
  import { SpeechJobsController, speechActive, speechPhase } from './jobs.svelte';
  import { speechProgress } from './progress';
  import { voicevoxUrl } from './connection';
  import { SpeechBatch } from './batch.svelte';
  import { Sbv2Profiles } from './profiles.svelte';
  import { QwenProfiles } from './qwen-profiles.svelte';
  import { IrodoriProfiles } from './irodori-profiles.svelte';
  import IrodoriProfilePicker from './IrodoriProfilePicker.svelte';
  import QwenProfilePicker from './QwenProfilePicker.svelte';
  import Sbv2ProfilePicker from './Sbv2ProfilePicker.svelte';
  import SpeechStartStatus from './SpeechStartStatus.svelte';
  let { editor }: { editor: EditorController } = $props();
  const jobs = (() => new SpeechJobsController(hostApi, editor.snapshot!.session_id))();
  const batch = new SpeechBatch(hostApi, jobs);
  const profiles = new Sbv2Profiles(hostApi);
  const qwenProfiles = new QwenProfiles(hostApi);
  const irodoriProfiles = new IrodoriProfiles(hostApi);
  let preferredUrl = voicevoxUrl();
  let open = $state(false), clipId = $state(''), baseUrl = $state(preferredUrl);
  let dialog = $state<HTMLDialogElement>();
  let resultSection = $state<HTMLElement>();
  let revealedJob = '';
  $effect(() => {
    const id = open ? jobs.selected?.job_id : undefined;
    if (!id) { revealedJob = ''; return; }
    if (id === revealedJob) return;
    revealedJob = id;
    void tick().then(() => {
      if (open && jobs.selected?.job_id === id) resultSection?.scrollIntoView({ block: 'nearest' });
    });
  });
  const clips = $derived(editor.snapshot?.project.clips.filter(c => c.content.type === 'speech') ?? []);
  const selectedClip = $derived(clips.find(c => c.id === clipId));
  function isEngine(clip: Clip | undefined, engine: 'sbv2' | 'qwen' | 'irodori') {
    const content = clip?.content;
    return content?.type === 'speech' && editor.snapshot?.project.characters
      .some(character => character.id === content.character_id && character.voice.type === engine);
  }
  const usesSbv2 = $derived(isEngine(selectedClip, 'sbv2'));
  const sbv2Waiting = $derived(clips.some(c => c.content.type === 'speech' && c.content.audio === null && isEngine(c, 'sbv2')));
  const usesQwen = $derived(isEngine(selectedClip, 'qwen'));
  const qwenWaiting = $derived(clips.some(c => c.content.type === 'speech' && c.content.audio === null && isEngine(c, 'qwen')));
  const usesIrodori = $derived(isEngine(selectedClip, 'irodori'));
  const irodoriWaiting = $derived(clips.some(c => c.content.type === 'speech' && c.content.audio === null && isEngine(c, 'irodori')));
  const validProfile = $derived(profiles.ready);
  const disabled = $derived(batch.running || jobs.busy || !!jobs.pending || editor.busy || editor.drafting || !!editor.pending);
  $effect(() => { if (open && dialog) dialog.showModal(); });
  $effect(() => {
    if (!open || batch.running || !jobs.selected || !speechActive(jobs.selected)) return;
    const timer = setInterval(() => { if (!jobs.busy && !jobs.pending) void jobs.refresh(); }, 1500);
    return () => clearInterval(timer);
  });
  $effect(() => {
    if (!batch.running) return;
    const timer = setInterval(() => { void batch.advance(); }, 1500);
    return () => clearInterval(timer);
  });
  onDestroy(() => { batch.stop(); jobs.dispose(); profiles.dispose(); qwenProfiles.dispose(); irodoriProfiles.dispose(); });
  function show() {
    const remembered = voicevoxUrl();
    if (remembered !== preferredUrl) { preferredUrl = remembered; baseUrl = remembered; }
    clipId = clips.find(c => c.id === editor.selected)?.id ?? clips[0]?.id ?? '';
    open = true; void jobs.list();
    if (clips.some(c => isEngine(c, 'sbv2'))) void profiles.refresh();
    if (clips.some(c => isEngine(c, 'qwen'))) void qwenProfiles.refresh();
    if (clips.some(c => isEngine(c, 'irodori'))) void irodoriProfiles.refresh();
  }
  function close() { if (!jobs.busy && !jobs.pending) { if (batch.running) batch.stop(); open = false; } }
  function label(id: string) {
    const clip = clips.find(c => c.id === id);
    return clip?.content.type === 'speech' ? clip.content.subtitle_text : locale.text('speechDeleted');
  }
</script>
<button disabled={editor.busy || editor.drafting || !!editor.pending} onclick={show}>{locale.text('speechSynthesis')}</button>
{#if open && editor.snapshot}
  <dialog use:containDialogFocus bind:this={dialog} aria-label={locale.text('speechSynthesis')} oncancel={e => { e.preventDefault(); close(); }}>
    <header><h2>{locale.text('speechSynthesis')}</h2><button disabled={jobs.busy || !!jobs.pending} onclick={close}>{locale.text('close')}</button></header>
    <p>{locale.text('speechIntro')}</p>
    <fieldset {disabled}>
      <label>{locale.text('speechTarget')}<select bind:value={clipId}>
        {#each clips as clip}<option value={clip.id}>{label(clip.id).slice(0, 80) || locale.text('speechNoSubtitle')}</option>{/each}
      </select></label>
      {#if !clips.length}<p>{locale.text('speechEmpty')}</p>{/if}
      {#if usesSbv2 || sbv2Waiting}<Sbv2ProfilePicker {profiles} {disabled} />{/if}
      {#if usesQwen || qwenWaiting}<QwenProfilePicker profiles={qwenProfiles} {disabled} />{/if}
      {#if usesIrodori || irodoriWaiting}<IrodoriProfilePicker profiles={irodoriProfiles} {disabled} />{/if}
      <details><summary>{locale.text('speechConnection')}</summary><label>{locale.text('speechUrl')}<input type="url" bind:value={baseUrl} /></label><LocalConnections /></details>
      <button disabled={!clips.some(c => c.id === clipId) || (usesSbv2 && !validProfile) || (usesQwen && !qwenProfiles.ready) || (usesIrodori && !irodoriProfiles.ready)} onclick={() => jobs.start(editor.snapshot!, clipId, baseUrl, profiles.selectedId, qwenProfiles.selectedId, irodoriProfiles.selectedId)}>{locale.text('speechGenerate')}</button>
      <button disabled={(sbv2Waiting && !validProfile) || (qwenWaiting && !qwenProfiles.ready) || (irodoriWaiting && !irodoriProfiles.ready) || (jobs.selected !== null && speechActive(jobs.selected)) || !clips.some(c => c.content.type === 'speech' && c.content.audio === null)}
        onclick={() => batch.start(editor.snapshot!, baseUrl, profiles.selectedId, qwenProfiles.selectedId, irodoriProfiles.selectedId)}>{locale.text('speechGenerateMissing')}</button>
      <p>{locale.text('speechBatchHint')}</p>
    </fieldset>
    {#if batch.total}<p role="status">{locale.text('speechBatchProgress').replace('{done}', String(batch.completed)).replace('{total}', String(batch.total))}{locale.text(batch.running ? 'speechBatchRunning' : 'speechBatchStopped')}</p>{/if}
    {#if batch.message}<p role="status">{batch.message}</p>{/if}
    {#if batch.running}<button onclick={() => batch.stop()}>{locale.text('speechStopNext')}</button>{/if}
    {#if jobs.pending}<SpeechStartStatus busy={jobs.busy} retry={() => { void jobs.retry(); }} />{/if}
    {#if jobs.error}<p role="alert">{jobs.error}</p>{/if}
    {#if jobs.busy && !jobs.pending}<p role="status">{locale.text('speechChecking')}</p>{/if}
    <section aria-label={locale.text('speechHistory')}>
      <header><h3>{locale.text('speechHistory')}</h3><button disabled={batch.running || jobs.busy || !!jobs.pending} onclick={() => jobs.list()}>{locale.text('speechRefreshList')}</button></header>
      <div class="history">{#each jobs.jobs as job (job.job_id)}<button class:selected={jobs.selected?.job_id === job.job_id} disabled={batch.running || jobs.busy || !!jobs.pending} onclick={() => jobs.refresh(job.job_id)}>{label(job.clip_id).slice(0, 60)} — {speechPhase[job.phase]}</button>{:else}<p>{locale.text('speechNoHistory')}</p>{/each}</div>
      {#if jobs.more}<button disabled={batch.running || jobs.busy || !!jobs.pending} onclick={() => jobs.list(true)}>{locale.text('speechMore')}</button>{/if}
    </section>
    {#if jobs.selected}
      {@const job = jobs.selected}
      <section bind:this={resultSection} aria-label={locale.text('speechResult')}>
        <h3>{speechPhase[job.phase]}</h3>
        {#if job.phase === 'running' && job.progress}<p role="status">{speechProgress(job.progress.stage, locale.current)}</p>{/if}
        <p>{locale.text('speechSourceRevision').replace('{target}', () => label(job.clip_id)).replace('{revision}', String(job.source_revision))}</p>
        {#if job.stale}<p class="notice">{locale.text('speechStale')}</p>{/if}
        {#if job.engine_settlement_required && !speechActive(job)}<p class="notice">{locale.text('speechUnsettled')}</p>{/if}
        {#if job.error}<p role="alert">{job.error.message}</p>{/if}
        {#if job.artifact}<p>{locale.text('speechSavedDuration').replace('{seconds}', (job.artifact.duration_us / 1e6).toFixed(3))}</p>{#if job.application.type === 'applied'}<p>{locale.text('speechAppliedRevision').replace('{revision}', String(job.application.revision))}</p>
          {#if job.application.overlapping_clip_ids.length}<p class="notice">{locale.text('speechOverlap').replace('{count}', String(job.application.overlapping_clip_ids.length))}</p>{/if}
        {:else if job.application.type === 'pending_parent'}<p class="notice">{locale.text('speechParent')}</p>
        {:else}<p class="notice">{locale.text('speechPending')}</p>{/if}
        <p>{locale.text('speechQuality')}</p><details><summary>{locale.text('speechOriginal')}</summary><code>{job.artifact.path}</code></details>{/if}
        <button disabled={jobs.busy || !!jobs.pending} onclick={() => jobs.refresh()}>{locale.text('speechRefresh')}</button>
        {#if speechActive(job)}<button disabled={jobs.busy || !!jobs.pending} onclick={() => jobs.cancel()}>{locale.text('speechCancel')}</button>{/if}
      </section>
    {/if}
  </dialog>
{/if}
<style>
  header { display:flex; justify-content:space-between; align-items:center; gap:12px; }
  header h3 { margin:0; }
  fieldset { border:1px solid var(--panel-border); border-radius:6px; padding:14px; }
  label { display:flex; flex-direction:column; gap:6px; margin-bottom:12px; }
  select,input { width:100%; }
  section { margin-top:18px; padding:14px; border:1px solid var(--panel-border); background:var(--background); border-radius:6px; }
  .history { display:flex; flex-direction:column; gap:6px; margin:12px 0; max-height:180px; overflow:auto; }
  .history button { text-align:left; overflow-wrap:anywhere; }
  .selected { border:2px solid var(--accent); }
  .notice { padding-left:10px; border-left:4px solid var(--accent); }
  code { overflow-wrap:anywhere; }
</style>
