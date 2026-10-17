<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { SessionSnapshot } from '@sungeki/api-client';
  import type { EditorController } from '../editor/controller.svelte';
  import type { SubmitCommands } from './commands';
  import SceneForm from './SceneForm.svelte';
  import GroupForm from './GroupForm.svelte';
  import SelectionForm from './SelectionForm.svelte';
  import RippleForm from './RippleForm.svelte';
  import EditReview from './EditReview.svelte';
  import { EditPlan } from './edit-plan.svelte';
  import { hostApi } from '../../shared/host-api';
  let { editor }: { editor: EditorController } = $props();
  let observed = $state.raw<SessionSnapshot>((() => editor.snapshot!)());
  let refreshGeneration = $state(0);
  const plan = new EditPlan(hostApi);
  let stale = $derived(observed.session_id !== editor.snapshot?.session_id || observed.revision !== editor.snapshot?.revision);
  let disabled = $derived(stale || editor.busy || !!editor.pending || editor.drafting);
  function refresh() { if (editor.snapshot) { plan.cancel(); observed = editor.snapshot; refreshGeneration++; } }
  const submit: SubmitCommands = async build => {
    if (disabled) return;
    try {
      await plan.prepare(observed, build());
    } catch (error) { editor.error = error instanceof Error ? error.message : String(error); }
  };
  async function confirm() {
    if (disabled || plan.busy || plan.stale(editor.snapshot) || !plan.request || !plan.preview || plan.error) return;
    await editor.request(plan.request);
    if (!editor.error && !editor.pending) refresh();
  }
</script>
<details class="structure">
  <summary>{locale.text('reviewTitle')}</summary>
  <p>{locale.text('reviewHint')}</p>
  {#if stale}<p role="alert">{locale.text('reviewStaleInput')}</p>{/if}
  {#if editor.drafting}<p>{locale.text('reviewDraftHint')}</p>{/if}
  <button disabled={editor.busy || !!editor.pending} onclick={refresh}>{locale.text('reviewRefresh')}</button>
  <fieldset hidden={!!plan.request} disabled={disabled || !!plan.request}>
    {#key refreshGeneration}
      <SceneForm project={observed.project} {submit} />
      <GroupForm project={observed.project} {submit} />
      <SelectionForm project={observed.project} {submit} />
      <RippleForm project={observed.project} selected={editor.selection} {submit} />
    {/key}
  </fieldset>
  {#if plan.request}<EditReview {plan} project={observed.project} stale={stale || plan.stale(editor.snapshot)} disabled={editor.busy || !!editor.pending || editor.drafting} {confirm} />{/if}
</details>
<style>
  .structure { border-top: 1px solid var(--border); padding-top: 16px; margin-top: 20px; }
  fieldset { border: 0; padding: 0; }
  button { padding: 8px 12px; }
</style>
