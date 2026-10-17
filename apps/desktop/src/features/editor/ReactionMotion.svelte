<script lang="ts">
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { Clip, SessionSnapshot } from '@sungeki/api-client';
  import type { EditorController } from './controller.svelte';
  import { hostApi } from '../../shared/host-api';
  import { EditPlan } from '../scenes/edit-plan.svelte';
  import EditReview from '../scenes/EditReview.svelte';
  import EditorNotices from './EditorNotices.svelte';
  let { editor, clip }: { editor: EditorController; clip: Clip } = $props();
  let dialog: HTMLDialogElement;
  let observed = $state.raw<SessionSnapshot | null>(null);
  let centerX = $state(0), centerY = $state(0), width = $state(120), height = $state(120);
  let startScale = $state(.6), fadeIn = $state(.08), fadeOut = $state(.15);
  const plan = new EditPlan(hostApi);
  let stale = $derived(!!observed && (observed.session_id !== editor.snapshot?.session_id || observed.revision !== editor.snapshot?.revision));
  let disabled = $derived(stale || editor.busy || !!editor.pending || editor.drafting);
  function refresh() {
    plan.cancel(); observed = editor.snapshot;
    const current = observed?.project.clips.find(c => c.id === clip.id);
    if (!current) return;
    const t = current.transform, scale = t.motion?.end_scale ?? 1;
    width = t.width * scale; height = t.height * scale;
    centerX = (t.motion?.end_x ?? t.x) + width / 2;
    centerY = (t.motion?.end_y ?? t.y) + height / 2;
    startScale = t.motion && scale >= 1 ? 1 / scale : .6;
    fadeIn = (t.fade?.in_us ?? Math.min(80000, current.duration_us / 4)) / 1e6;
    fadeOut = (t.fade?.out_us ?? Math.min(150000, current.duration_us / 4)) / 1e6;
  }
  async function preview(event: SubmitEvent) {
    event.preventDefault();
    if (disabled || !observed || [centerX, centerY, width, height, startScale, fadeIn, fadeOut].some(n => !Number.isFinite(n))) return;
    await plan.prepare(observed, [{ type: 'set_reaction_motion', clip_id: clip.id,
      center_x: centerX, center_y: centerY, width, height, start_scale: startScale,
      fade: { in_us: Math.round(fadeIn * 1e6), out_us: Math.round(fadeOut * 1e6) } }]);
  }
  async function confirm() {
    if (disabled || plan.busy || plan.stale(editor.snapshot) || !plan.request || !plan.preview || plan.error) return;
    await editor.request(plan.request);
    if (!editor.error && !editor.pending) { plan.cancel(); dialog.close(); }
  }
</script>
<button type="button" disabled={editor.busy || !!editor.pending || editor.drafting}
  onclick={() => { refresh(); dialog.showModal(); }}>{locale.text('reactionTitle')}</button>
<dialog use:containDialogFocus bind:this={dialog} onclose={() => plan.cancel()} aria-label={locale.text('reactionTitle')}>
  <header><h2>{locale.text('reactionTitle')}</h2><button type="button" onclick={() => dialog.close()}>{locale.text('assetActionClose')}</button></header>
  <EditorNotices {editor} />
  <p>{locale.text('reactionReplaceHint')}</p>
  <p>{locale.text('reactionDurationHint')}</p>
  {#if stale}<p role="alert">{locale.text('fitTimingStale')}</p>{/if}
  <button type="button" disabled={editor.busy || !!editor.pending} onclick={refresh}>{locale.text('fitTimingReload')}</button>
  {#if observed && plan.request}
    <EditReview {plan} project={observed.project} stale={stale || plan.stale(editor.snapshot)} {disabled} {confirm} />
  {:else}
    <form onsubmit={preview}><fieldset disabled={disabled}>
      <label>{locale.text('reactionCenterX')}<input type="number" required min="-1000000" max="1000000" step="any" bind:value={centerX} /></label>
      <label>{locale.text('reactionCenterY')}<input type="number" required min="-1000000" max="1000000" step="any" bind:value={centerY} /></label>
      <label>{locale.text('reactionWidth')}<input type="number" required min="1" max="32768" step="any" bind:value={width} /></label>
      <label>{locale.text('reactionHeight')}<input type="number" required min="1" max="32768" step="any" bind:value={height} /></label>
      <label>{locale.text('reactionScale')}<input type="number" required min="0.01" max="1" step="any" bind:value={startScale} /></label>
      <label>{locale.text('reactionFadeIn')}<input type="number" required min="0" step="0.001" bind:value={fadeIn} /></label>
      <label>{locale.text('reactionFadeOut')}<input type="number" required min="0" step="0.001" bind:value={fadeOut} /></label>
      <button disabled={plan.busy}>{locale.text('assetActionReview')}</button>
    </fieldset></form>
  {/if}
</dialog>
<style>
  header { display:flex; justify-content:space-between; align-items:center; gap:20px; }
  dialog { max-width:760px; }
  fieldset { border:0; padding:12px 0; display:grid; gap:10px; }
  label { display:flex; justify-content:space-between; gap:16px; }
</style>
