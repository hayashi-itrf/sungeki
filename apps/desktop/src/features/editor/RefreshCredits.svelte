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
  const plan = new EditPlan(hostApi);
  let stale = $derived(!!observed && (observed.session_id !== editor.snapshot?.session_id || observed.revision !== editor.snapshot?.revision));
  let disabled = $derived(stale || editor.busy || !!editor.pending || editor.drafting);
  let incomplete = $derived(!!plan.preview?.entries.some(e => e.before?.truncated || e.after?.truncated));
  async function prepare() {
    if (disabled || !observed || !clip.scene_id) return;
    await plan.prepare(observed, [{ type: 'refresh_scene_credits', clip_id: clip.id, scene_id: clip.scene_id }]);
  }
  async function confirm() {
    if (disabled || incomplete || plan.busy || plan.stale(editor.snapshot) || !plan.request || !plan.preview || plan.error) return;
    await editor.request(plan.request);
    if (!editor.error && !editor.pending) { plan.cancel(); dialog.close(); }
  }
</script>
<button type="button" disabled={editor.busy || !!editor.pending || editor.drafting}
  onclick={() => { plan.cancel(); observed = editor.snapshot; dialog.showModal(); }}>{locale.text('refreshCreditsTitle')}</button>
<dialog use:containDialogFocus bind:this={dialog} onclose={() => plan.cancel()} aria-label={locale.text('refreshCreditsTitle')}>
  <header><h2>{locale.text('refreshCreditsTitle')}</h2><button onclick={() => dialog.close()}>{locale.text('assetActionClose')}</button></header>
  <EditorNotices {editor} />
  <p>{locale.text('refreshCreditsReplace')}</p>
  <p>{locale.text('refreshCreditsHint')}</p>
  {#if stale}<p role="alert">{locale.text('refreshCreditsStale')}</p>{/if}
  {#if observed && plan.request}
    {#if incomplete}<p role="alert">{locale.text('refreshCreditsIncomplete')}</p>{/if}
    <EditReview {plan} project={observed.project} stale={stale || plan.stale(editor.snapshot)} disabled={disabled || incomplete} {confirm} />
  {:else}<button disabled={disabled} onclick={prepare}>{locale.text('assetActionReview')}</button>{/if}
</dialog>
<style>
  header { display:flex; justify-content:space-between; align-items:center; gap:20px; }
  dialog { max-width:760px; }
</style>
