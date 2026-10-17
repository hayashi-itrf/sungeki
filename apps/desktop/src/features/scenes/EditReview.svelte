<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { Project } from '@sungeki/api-client';
  import { describeCommand } from './command-description';
  import type { EditPlan } from './edit-plan.svelte';
  import { differenceLabel, differenceSubject, differenceValue } from '../editor/difference-labels';
  let { plan, project, stale, disabled, confirm }: { plan: EditPlan; project: Project; stale: boolean; disabled: boolean; confirm: () => Promise<void> } = $props();
</script>
<section aria-label={locale.text('reviewRegion')}>
  <h3>{locale.text('reviewHeading')}</h3>
  <p>{locale.text('reviewNotApplied')}</p>
  {#if plan.request?.edit.action.type === 'apply'}
    <details open><summary>{locale.text('reviewOperations')}</summary>
      {#each plan.request.edit.action.commands as command}<p>{describeCommand(command, project)}</p>{/each}
    </details>
  {/if}
  {#if stale}<p role="alert">{locale.text('reviewStale')}</p>{/if}
  {#if plan.busy}<p role="status">{locale.text('reviewLoading')}</p>{/if}
  {#if plan.error}<p role="alert">{plan.error}</p><button disabled={plan.busy || stale} onclick={() => plan.page(false)}>{locale.text('reviewRetry')}</button>{/if}
  {#if plan.preview}
    <p>{locale.text('reviewSummary').replace('{count}', String(plan.preview.total_changes)).replace('{before}', String(plan.preview.before_duration_us / 1e6)).replace('{after}', String(plan.preview.after_duration_us / 1e6))}</p>
    {#if plan.preview.no_op}<p>{locale.text('reviewNoOp')}</p>{/if}
    {#each plan.preview.entries as entry}
      <div class="change">
        <strong>{differenceLabel(entry)}</strong>
        {#if entry.entity_id}<p>{entry.kind === 'added' ? locale.text('reviewNewEntity') : differenceSubject(entry, project)}</p>{/if}
        <p>{locale.text('reviewBefore')}{differenceValue(entry.before, entry)}</p>
        <p>{locale.text('reviewAfter')}{differenceValue(entry.after, entry)}</p>
      </div>
    {/each}
    {#if plan.preview.next_offset !== null}<button disabled={plan.busy || stale} onclick={() => plan.page(true)}>{locale.text('reviewMore')}</button>{/if}
    <button disabled={disabled || plan.busy || stale || plan.preview.no_op || !!plan.error} onclick={confirm}>{locale.text('reviewApply')}</button>
  {/if}
  <button disabled={disabled} onclick={() => plan.cancel()}>{locale.text('reviewCancel')}</button>
</section>
<style>
  section { border: 1px solid var(--border); padding: 16px; margin-top: 16px; }
  .change { border-top: 1px solid var(--border); padding: 8px 0; overflow-wrap: anywhere; }
  .change p { white-space: pre-wrap; } button { padding: 8px 12px; margin: 4px; }
</style>
