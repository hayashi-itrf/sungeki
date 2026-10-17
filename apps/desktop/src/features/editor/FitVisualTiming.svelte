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
  let first = $state('');
  let last = $state('');
  const plan = new EditPlan(hostApi);
  let speeches = $derived(observed?.project.clips.filter(c => c.content.type === 'speech')
    .sort((a, b) => a.start_us - b.start_us || a.id.localeCompare(b.id)) ?? []);
  let stale = $derived(!!observed && (observed.session_id !== editor.snapshot?.session_id || observed.revision !== editor.snapshot?.revision));
  let disabled = $derived(stale || editor.busy || !!editor.pending || editor.drafting);
  function refresh() {
    plan.cancel(); observed = editor.snapshot;
    first = speeches[0]?.id ?? ''; last = first;
  }
  async function preview(event: SubmitEvent) {
    event.preventDefault();
    if (disabled || !observed || !first || !last) return;
    await plan.prepare(observed, [{ type: 'fit_visual_to_speech', clip_id: clip.id,
      first_speech_id: first, last_speech_id: last }]);
  }
  async function confirm() {
    if (disabled || plan.busy || plan.stale(editor.snapshot) || !plan.request || !plan.preview || plan.error) return;
    await editor.request(plan.request);
    if (!editor.error && !editor.pending) { plan.cancel(); dialog.close(); }
  }
</script>
<button type="button" disabled={editor.busy || !!editor.pending || editor.drafting}
  onclick={() => { refresh(); dialog.showModal(); }}>{locale.text('fitTimingTitle')}</button>
<dialog use:containDialogFocus bind:this={dialog} onclose={() => plan.cancel()} aria-label={locale.text('fitTimingTitle')}>
  <header><h2>{locale.text('fitTimingTitle')}</h2><button type="button" onclick={() => dialog.close()}>{locale.text('assetActionClose')}</button></header>
  <EditorNotices {editor} />
  <p>{locale.text('fitTimingRange')}</p>
  <p>{locale.text('fitTimingHint')}</p>
  {#if stale}<p role="alert">{locale.text('fitTimingStale')}</p>{/if}
  <button type="button" disabled={editor.busy || !!editor.pending} onclick={refresh}>{locale.text('fitTimingReload')}</button>
  {#if observed && plan.request}
    <EditReview {plan} project={observed.project} stale={stale || plan.stale(editor.snapshot)} {disabled} {confirm} />
  {:else}
    <form onsubmit={preview}>
      <fieldset disabled={disabled}>
        <label>{locale.text('fitTimingFirst')}<select required bind:value={first}>
          {#each speeches as speech (speech.id)}
            {#if speech.content.type === 'speech'}<option value={speech.id}>{speech.start_us / 1e6}{locale.text('fitTimingSeconds')}{speech.content.subtitle_text}{speech.content.audio ? '' : locale.text('fitTimingNoAudio')}</option>{/if}
          {/each}
        </select></label>
        <label>{locale.text('fitTimingLast')}<select required bind:value={last}>
          {#each speeches as speech (speech.id)}
            {#if speech.content.type === 'speech'}<option value={speech.id}>{speech.start_us / 1e6}{locale.text('fitTimingSeconds')}{speech.content.subtitle_text}{speech.content.audio ? '' : locale.text('fitTimingNoAudio')}</option>{/if}
          {/each}
        </select></label>
        <button disabled={!first || !last}>{locale.text('assetActionReview')}</button>
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
