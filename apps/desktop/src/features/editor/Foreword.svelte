<script lang="ts">
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { SessionSnapshot } from '@sungeki/api-client';
  import type { EditorController } from './controller.svelte';
  import { hostApi } from '../../shared/host-api';
  import { EditPlan } from '../scenes/edit-plan.svelte';
  import EditReview from '../scenes/EditReview.svelte';
  import EditorNotices from './EditorNotices.svelte';
  let { editor }: { editor: EditorController } = $props();
  let dialog: HTMLDialogElement;
  let observed = $state.raw<SessionSnapshot | null>(null);
  let text = $state(''), background = $state(''), seconds = $state(8);
  const plan = new EditPlan(hostApi);
  let stale = $derived(!!observed && (observed.revision !== editor.snapshot?.revision || observed.session_id !== editor.snapshot?.session_id));
  let disabled = $derived(stale || editor.busy || !!editor.pending || editor.drafting);
  let images = $derived(observed?.project.assets.filter(a => a.probe.type === 'image') ?? []);
  function refresh() { plan.cancel(); observed = editor.snapshot; text = ''; background = ''; seconds = 8; }
  async function preview(event: SubmitEvent) {
    event.preventDefault();
    if (!observed || disabled || !Number.isFinite(seconds) || seconds < 1.6) return;
    await plan.prepare(observed, [{ type: 'prepend_foreword', template: {
      preset_id: 'foreword-wide-v1', background_asset_id: background, text, duration_us: Math.round(seconds * 1e6),
      scene_id: crypto.randomUUID(), track_id: crypto.randomUUID(), clip_ids: Array.from({ length: 4 }, () => crypto.randomUUID()),
    } }]);
  }
  async function confirm() {
    if (disabled || plan.busy || plan.stale(editor.snapshot) || !plan.request || !plan.preview || plan.error) return;
    await editor.request(plan.request);
    if (!editor.error && !editor.pending) { plan.cancel(); dialog.close(); }
  }
</script>
<button disabled={editor.busy || !!editor.pending || editor.drafting} onclick={() => { refresh(); dialog.showModal(); }}>{locale.text('forewordAdd')}</button>
<dialog use:containDialogFocus bind:this={dialog} onclose={() => plan.cancel()} aria-label={locale.text('forewordTitle')}>
  <header><h2>{locale.text('forewordTitle')}</h2><button onclick={() => dialog.close()}>{locale.text('close')}</button></header>
  <EditorNotices {editor} />
  <p>{locale.text('forewordCanvasHint')}</p>
  <p>{locale.text('forewordShiftHint')}</p>
  <p>{locale.text('forewordCreditHint')}</p>
  {#if stale}<p role="alert">{locale.text('forewordStale')}</p>{/if}
  <button disabled={editor.busy || !!editor.pending} onclick={refresh}>{locale.text('forewordRefresh')}</button>
  {#if observed && plan.request}
    <EditReview {plan} project={observed.project} stale={stale || plan.stale(editor.snapshot)} {disabled} {confirm} />
  {:else}
    <form onsubmit={preview}><fieldset disabled={disabled}>
      <label>{locale.text('forewordBackground')}<select required bind:value={background}><option value="">{locale.text('forewordSelect')}</option>{#each images as image}<option value={image.id}>{image.name}{image.credit.trim() ? '' : locale.text('forewordMissingCredit')}</option>{/each}</select></label>
      <label>{locale.text('forewordText')}<textarea required rows="4" bind:value={text}></textarea></label>
      <label>{locale.text('forewordDuration')}<input type="number" required min="1.6" max="604800" step="0.001" bind:value={seconds} /></label>
      <button type="submit" disabled={plan.busy || !background || !text.trim()}>{locale.text('forewordPreview')}</button>
    </fieldset></form>
  {/if}
</dialog>
<style>
  dialog { width:min(680px,90vw); max-height:85vh; overflow:auto; color:var(--text); background:var(--surface); border:1px solid var(--border); border-radius:8px; }
  header { display:flex; justify-content:space-between; align-items:center; }
  label { display:flex; flex-direction:column; gap:6px; margin:12px 0; }
  p { font-size:13px; }
  fieldset { border:0; padding:0; }
  textarea { width:100%; box-sizing:border-box; }
</style>
