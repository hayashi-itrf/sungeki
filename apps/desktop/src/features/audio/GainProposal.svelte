<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { onDestroy } from 'svelte';
  import type { LoudnessJob, SpeechGainProposal } from '@sungeki/api-client';
  import type { EditorController } from '../editor/controller.svelte';
  import { hostApi } from '../../shared/host-api';
  import { proposalCurrent } from './proposal';
  let { editor, job, clipId, disabled }: { editor: EditorController; job: LoudnessJob; clipId: string; disabled: boolean } = $props();
  let target = $state<number>(), peak = $state<number>();
  let proposal = $state<SpeechGainProposal | null>(null), busy = $state(false), error = $state(''), applied = $state(false);
  let alive = true;
  onDestroy(() => { alive = false; });
  const blocked = $derived(disabled || busy || editor.busy || editor.drafting || !!editor.pending);
  const stale = $derived(!editor.snapshot || job.stale || job.source_revision !== editor.snapshot.revision);
  const valid = $derived(target !== undefined && Number.isFinite(target) && target > -70 && target <= 0
    && peak !== undefined && Number.isFinite(peak) && peak >= -60 && peak <= 0);
  const current = $derived(!!proposal && !!editor.snapshot && proposalCurrent(proposal, editor.snapshot, job, clipId, target, peak));
  async function propose() {
    if (blocked || stale || !valid || !editor.snapshot) return;
    const snapshot = editor.snapshot, selected = job, clip = clipId, t = target!, ceiling = peak!;
    busy = true; error = ''; proposal = null; applied = false;
    try {
      const response = await hostApi.session({ type: 'propose_speech_gain', args: {
        session_id: snapshot.session_id, expected_revision: snapshot.revision, job_id: selected.job_id,
        clip_id: clip, target_lufs: t, max_true_peak_db: ceiling,
      } });
      if (!alive) return;
      if (response.type !== 'speech_gain_proposed' || !proposalCurrent(response.proposal, snapshot, selected, clip, t, ceiling)) {
        throw new Error(locale.text('loudnessProposalMismatch'));
      }
      proposal = response.proposal;
    } catch (e) { if (alive) error = e && typeof e === 'object' && 'message' in e ? String(e.message) : String(e); }
    finally { if (alive) busy = false; }
  }
  async function apply() {
    if (blocked || !current || !proposal) return;
    const reviewed = proposal;
    await editor.apply([reviewed.command]);
    const confirmed = editor.snapshot?.project.clips.find(c => c.id === reviewed.clip_id);
    if (alive && !editor.error && !editor.pending && editor.snapshot
      && editor.snapshot.revision > reviewed.source_revision && confirmed?.content.type === 'speech'
      && confirmed.content.gain_db === reviewed.estimate.gain_db) applied = true;
  }
</script>
<section aria-label={locale.text('loudnessProposal')}>
  <h3>{locale.text('loudnessMatchTarget')}</h3>
  <p>{locale.text('loudnessProposalHint')}</p>
  <div class="targets">
    <label>{locale.text('loudnessTarget')}<input type="number" step="any" min="-70" max="0" bind:value={target} disabled={busy} /></label>
    <label>{locale.text('loudnessCeiling')}<input type="number" step="any" min="-60" max="0" bind:value={peak} disabled={busy} /></label>
  </div>
  <button disabled={blocked || stale || !valid} onclick={propose}>{locale.text('loudnessPropose')}</button>
  {#if busy}<p role="status">{locale.text('loudnessChecking')}</p>{/if}
  {#if error}<p role="alert">{error}</p>{/if}
  {#if proposal}
    <p>{locale.text('loudnessCurrent')}{(proposal.current_gain_db ?? 0).toFixed(2)} dB → {locale.text('loudnessProposed')}<strong>{proposal.estimate.gain_db.toFixed(2)} dB</strong></p>
    <p>{locale.text('loudnessPredicted')}{proposal.estimate.predicted_lufs.toFixed(1)} LUFS ／ {proposal.estimate.predicted_true_peak_db.toFixed(1)} dBTP</p>
    {#if proposal.estimate.limited_by_peak || proposal.estimate.limited_by_gain_range}<p class="notice">{locale.text(proposal.estimate.limited_by_peak ? 'loudnessPeakLimited' : 'loudnessGainLimited')}</p>{/if}
    {#if !current && !applied}<p role="alert">{locale.text('loudnessProposalStale')}</p>{/if}
    <button disabled={blocked || !current} onclick={apply}>{locale.text('loudnessApply')}</button>
  {/if}
  {#if applied}<p role="status">{locale.text('loudnessApplied')}</p>{/if}
  {#if editor.error}<p role="alert">{editor.error}</p>{/if}
  {#if editor.pending}<p role="alert">{locale.text('loudnessApplyPending')}</p>{/if}
  <p>{locale.text('loudnessApplyHint')}</p>
</section>
<style>
  section { margin-top:18px; padding:14px; border:2px solid var(--panel-border); border-radius:6px; background:var(--background); }
  .targets { display:flex; flex-wrap:wrap; gap:12px; margin-bottom:12px; }
  label { display:flex; flex-direction:column; gap:6px; flex:1; min-width:150px; }
  input { width:100%; }
  .notice { padding:10px; border-left:4px solid var(--accent); }
</style>
