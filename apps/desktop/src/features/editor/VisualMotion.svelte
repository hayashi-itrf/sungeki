<script lang="ts">
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { locale } from '../../shared/i18n/locale.svelte';
  import { motionCommand } from './motion-command';
  import type { Clip, SessionSnapshot, MotionEasing } from '@sungeki/api-client';
  import type { EditorController } from './controller.svelte';
  import { hostApi } from '../../shared/host-api';
  import { EditPlan } from '../scenes/edit-plan.svelte';
  import EditReview from '../scenes/EditReview.svelte';
  import EditorNotices from './EditorNotices.svelte';
  let { editor, clip }: { editor: EditorController; clip: Clip } = $props();
  let dialog: HTMLDialogElement;
  let observed = $state.raw<SessionSnapshot | null>(null);
  let enabled = $state(false), endX = $state(0), endY = $state(0), scale = $state(1);
  let easing = $state<MotionEasing>('ease_in_out');
  const plan = new EditPlan(hostApi);
  let stale = $derived(!!observed && (observed.session_id !== editor.snapshot?.session_id || observed.revision !== editor.snapshot?.revision));
  let disabled = $derived(stale || editor.busy || !!editor.pending || editor.drafting);
  function refresh() {
    plan.cancel(); observed = editor.snapshot;
    const current = observed?.project.clips.find(c => c.id === clip.id);
    const t = current?.transform; enabled = !!t?.motion; endX = t?.motion?.end_x ?? t?.x ?? 0;
    endY = t?.motion?.end_y ?? t?.y ?? 0; scale = t?.motion?.end_scale ?? 1; easing = t?.motion?.easing ?? 'ease_in_out';
  }
  async function preview(event: SubmitEvent) {
    event.preventDefault();
    if (disabled || !observed || (enabled && [endX, endY, scale].some(n => !Number.isFinite(n)))) return;
    const current = observed.project.clips.find(c => c.id === clip.id);
    if (!current) return;
    await plan.prepare(observed, [motionCommand(current, enabled ? { end_x: endX, end_y: endY, end_scale: scale, easing } : undefined)]);
  }
  async function confirm() {
    if (disabled || plan.busy || plan.stale(editor.snapshot) || !plan.request || !plan.preview || plan.error) return;
    await editor.request(plan.request);
    if (!editor.error && !editor.pending) { plan.cancel(); dialog.close(); }
  }
</script>
<button type="button" disabled={editor.busy || !!editor.pending || editor.drafting}
  onclick={() => { refresh(); dialog.showModal(); }}>{locale.text('motionTitle')}</button>
<dialog use:containDialogFocus bind:this={dialog} onclose={() => plan.cancel()} aria-label={locale.text('motionTitle')}>
  <header><h2>{locale.text('motionTitle')}</h2><button type="button" onclick={() => dialog.close()}>{locale.text('assetActionClose')}</button></header>
  <EditorNotices {editor} />
  <p>{locale.text('motionDurationHint')}</p>
  <p>{locale.text('motionPositionHint')}</p>
  {#if stale}<p role="alert">{locale.text('fitTimingStale')}</p>{/if}
  <button type="button" disabled={editor.busy || !!editor.pending} onclick={refresh}>{locale.text('fitTimingReload')}</button>
  {#if observed && plan.request}
    <EditReview {plan} project={observed.project} stale={stale || plan.stale(editor.snapshot)} {disabled} {confirm} />
  {:else}
    <form onsubmit={preview}>
      <fieldset disabled={disabled}>
        <label><input type="checkbox" bind:checked={enabled} />{locale.text('motionEnabled')}</label>
        <fieldset disabled={!enabled}>
          <label>{locale.text('effectsEndX')}<input type="number" required min="-1000000" max="1000000" step="any" bind:value={endX} /></label>
          <label>{locale.text('effectsEndY')}<input type="number" required min="-1000000" max="1000000" step="any" bind:value={endY} /></label>
          <label>{locale.text('motionScale')}<input type="number" required min="0.000001" step="any" bind:value={scale} /></label>
          <label>{locale.text('effectsEasing')}<select bind:value={easing}><option value="ease_in_out">{locale.text('effectsSmooth')}</option><option value="linear">{locale.text('effectsLinear')}</option></select></label>
        </fieldset>
        <button disabled={plan.busy}>{locale.text('assetActionReview')}</button>
      </fieldset>
    </form>
  {/if}
</dialog>
<style>
  header { display:flex; justify-content:space-between; align-items:center; gap:20px; }
  dialog { max-width:760px; }
  fieldset { border:0; padding:12px 0; }
  label { display:block; margin:10px 0; }
  select { display:block; width:100%; margin-top:6px; }
</style>
