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
  let sceneId = $state(''), inSeconds = $state(0), outSeconds = $state(0);
  const plan = new EditPlan(hostApi);
  let stale = $derived(!!observed && (observed.revision !== editor.snapshot?.revision || observed.session_id !== editor.snapshot?.session_id));
  let disabled = $derived(stale || editor.busy || !!editor.pending || editor.drafting);
  let scenes = $derived(observed?.project.scenes ?? []);
  function selectScene() { const scene = scenes.find(s => s.id === sceneId); inSeconds = (scene?.fade?.in_us ?? 0) / 1e6; outSeconds = (scene?.fade?.out_us ?? 0) / 1e6; }
  function refresh() { plan.cancel(); observed = editor.snapshot; sceneId = observed?.project.scenes[0]?.id ?? ''; selectScene(); }
  async function preview(event: SubmitEvent) {
    event.preventDefault();
    if (!observed || disabled || !sceneId || !Number.isFinite(inSeconds) || !Number.isFinite(outSeconds)) return;
    await plan.prepare(observed, [{ type: 'set_scene_fade', scene_id: sceneId,
      fade: inSeconds === 0 && outSeconds === 0 ? null : { in_us: Math.round(inSeconds * 1e6), out_us: Math.round(outSeconds * 1e6) } }]);
  }
  async function confirm() {
    if (disabled || plan.busy || plan.stale(editor.snapshot) || !plan.request || !plan.preview || plan.error) return;
    await editor.request(plan.request);
    if (!editor.error && !editor.pending) { plan.cancel(); dialog.close(); }
  }
</script>
<button disabled={editor.busy || !!editor.pending || editor.drafting} onclick={() => { refresh(); dialog.showModal(); }}>{locale.text('sceneFadeTitle')}</button>
<dialog use:containDialogFocus bind:this={dialog} onclose={() => plan.cancel()} aria-label={locale.text('sceneFadeTitle')}>
  <header><h2>{locale.text('sceneFadeTitle')}</h2><button onclick={() => dialog.close()}>{locale.text('close')}</button></header>
  <EditorNotices {editor} />
  <p>{locale.text('sceneFadeHint')}</p>
  <p>{locale.text('sceneFadeDurationHint')}</p>
  {#if stale}<p role="alert">{locale.text('sceneFadeStale')}</p>{/if}
  <button disabled={editor.busy || !!editor.pending} onclick={refresh}>{locale.text('reviewRefresh')}</button>
  {#if observed && plan.request}
    <EditReview {plan} project={observed.project} stale={stale || plan.stale(editor.snapshot)} {disabled} {confirm} />
  {:else}
    <form onsubmit={preview}><fieldset disabled={disabled}>
      <label>{locale.text('sceneFadeScene')}<select required bind:value={sceneId} onchange={selectScene}>{#each scenes as scene}<option value={scene.id}>{locale.text('sceneFadeDuration').replace('{seconds}', String(scene.duration_us / 1e6)).replace('{name}', scene.name)}</option>{/each}</select></label>
      <label>{locale.text('sceneFadeIn')}<input type="number" required min="0" max="604800" step="0.001" bind:value={inSeconds} /></label>
      <label>{locale.text('sceneFadeOut')}<input type="number" required min="0" max="604800" step="0.001" bind:value={outSeconds} /></label>
      <button type="submit" disabled={plan.busy || !sceneId}>{locale.text('sceneFadePreview')}</button>
    </fieldset></form>
  {/if}
</dialog>
<style>
  dialog { width:min(680px,90vw); max-height:85vh; overflow:auto; color:var(--text); background:var(--surface); border:1px solid var(--border); border-radius:8px; }
  header { display:flex; justify-content:space-between; align-items:center; }
  label { display:flex; flex-direction:column; gap:6px; margin:12px 0; }
  p { font-size:13px; }
  fieldset { border:0; padding:0; }

</style>
