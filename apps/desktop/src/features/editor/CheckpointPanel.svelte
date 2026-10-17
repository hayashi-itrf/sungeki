<script lang="ts">
  import { differenceLabel, differenceSubject, differenceValue } from './difference-labels';
  import { untrack } from 'svelte';
  import { locale } from '../../shared/i18n/locale.svelte';
  import { checkpointEnglish, checkpointJapanese } from './checkpoint-messages';
  import ReviewPanel from '../review/ReviewPanel.svelte';
  import CollectionPanel from '../collection/CollectionPanel.svelte';
  import type { CheckpointSummary, CheckpointComparison, SessionRequest } from '@sungeki/api-client';
  import { hostApi } from '../../shared/host-api';
  import type { EditorController } from './controller.svelte';
  let { editor }: { editor: EditorController } = $props();
  const sessionId = (() => editor.snapshot!.session_id)();
  const text = $derived(locale.current === 'en' ? checkpointEnglish : checkpointJapanese);
  let name = $state((locale.current === 'en' ? checkpointEnglish : checkpointJapanese).defaultName);
  let deleteCandidate = $state('');
  let pendingDelete = $state('');
  let items = $state<CheckpointSummary[]>([]);
  let selected = $state('');
  let target = $state('');
  let next = $state<string | null>(null);
  let comparison = $state<CheckpointComparison | null>(null);
  let busy = $state(false);
  let error = $state('');
  let restoredRevision = $state<number | null>(null);
  let pendingCreate = $state.raw<SessionRequest | null>(null);
  let listing = $state(false);
  let listGeneration = 0;
  let stale = $derived(comparison?.target_revision != null && comparison.target_revision !== editor.snapshot?.revision);
  $effect(() => { editor.checkpointResetGeneration; untrack(() => {
    selected = ''; target = ''; comparison = null; deleteCandidate = '';
  }); });
  $effect(() => { editor.checkpointGeneration; untrack(() => { void list(false); }); });

  async function list(more: boolean) {
    const generation = ++listGeneration;
    listing = true;
    try {
      const response = await hostApi.session({ type: 'checkpoint', session_id: sessionId,
        request: { type: 'list', after: more ? next : null, limit: 20 } });
      if (generation !== listGeneration) return;
      if (response.type !== 'checkpoints') throw new Error(text.listError);
      const oldSelected = items.filter(c => c.id === selected || c.id === target);
      const combined = more ? [...items, ...response.checkpoints] : [...response.checkpoints, ...oldSelected];
      items = [...new Map(combined.map(c => [c.id, c])).values()];
      next = response.next_after;
    } catch (e) { if (generation === listGeneration) error = message(e); }
    finally { if (generation === listGeneration) listing = false; }
  }
  function message(e: unknown) { return e && typeof e === 'object' && 'message' in e ? String(e.message) : String(e); }
  async function create() {
    busy = true; error = '';
    const request: SessionRequest = pendingCreate ?? { type: 'checkpoint', session_id: sessionId,
      request: { type: 'create', checkpoint_id: crypto.randomUUID(), name, expected_revision: editor.snapshot!.revision } };
    try {
      const response = await hostApi.session(request);
      if (response.type !== 'checkpoint_created') throw new Error(text.createError);
      pendingCreate = null; selected = response.checkpoint.id; comparison = null;
      items = [...items.filter(c => c.id !== selected), response.checkpoint];
      await list(false);
    } catch (e) {
      const code = e && typeof e === 'object' && 'code' in e ? e.code : null;
      if (code === 'HOST_UNAVAILABLE' || code === 'IO_ERROR' || !code) pendingCreate = request;
      else pendingCreate = null;
      error = message(e);
    } finally { busy = false; }
  }
  async function compare(more = false) {
    busy = true; error = '';
    const selectedAtStart = selected; const targetAtStart = target;
    const revision = more && comparison?.target_revision != null ? comparison.target_revision : editor.snapshot!.revision;
    try {
      const response = await hostApi.session({ type: 'checkpoint', session_id: sessionId, request: {
        type: 'compare', checkpoint_id: selected, target: target ? { type: 'checkpoint', checkpoint_id: target } : { type: 'current', expected_revision: revision },
        offset: more ? comparison?.next_offset ?? 0 : 0, limit: 20,
      } });
      if (selected !== selectedAtStart || target !== targetAtStart) return;
      if (response.type !== 'checkpoint_compared') throw new Error(text.compareError);
      comparison = more && comparison ? { ...response.comparison, entries: [...comparison.entries, ...response.comparison.entries] } : response.comparison;
    } catch (e) { error = message(e); } finally { busy = false; }
  }
  async function remove(id: string) {
    busy = true; error = '';
    try {
      const response = await hostApi.session({ type: 'checkpoint', session_id: sessionId,
        request: { type: 'delete', checkpoint_id: id } });
      if (response.type !== 'checkpoint_deleted') throw new Error(text.deleteError);
      pendingDelete = ''; deleteCandidate = ''; comparison = null;
      items = items.filter(c => c.id !== id);
      if (selected === id) selected = '';
      if (target === id) target = '';
      await list(false);
    } catch (e) {
      const code = e && typeof e === 'object' && 'code' in e ? e.code : null;
      pendingDelete = code === 'HOST_UNAVAILABLE' || code === 'IO_ERROR' || !code ? id : '';
      error = message(e);
    } finally { busy = false; }
  }
  async function restore() {
    const operationId = crypto.randomUUID();
    await editor.request({ type: 'checkpoint', session_id: sessionId, request: { type: 'restore',
      checkpoint_id: selected, operation_id: operationId, expected_revision: editor.snapshot!.revision } });
    if (!editor.error && !editor.pending && editor.snapshot?.undo_operation_id === operationId) {
      comparison = null;
      restoredRevision = editor.snapshot.revision;
    }
  }
</script>
<details class="checkpoints">
  <summary>{text.title}</summary>
  <p>{text.intro}</p>
  <p>{text.assetNotice}</p>
  <CollectionPanel {editor} includeHistory buttonLabel={text.inspectAssets}
    checkpointNames={Object.fromEntries(items.map(item => [item.id, item.name]))} />
  {#if error}<p role="alert">{error}</p>{/if}
  {#if restoredRevision !== null && restoredRevision === editor.snapshot?.revision}
    <p role="status">{text.restored}</p>
    <p>{text.reviewRestored}</p>
    <ReviewPanel {editor} />
  {/if}
  <fieldset disabled={busy || editor.busy || !!editor.pending}>
    <label>{text.name} <input bind:value={name} maxlength="200" disabled={!!pendingCreate} /></label>
    <button disabled={!name.trim() || editor.drafting} onclick={create}>{pendingCreate ? text.retryCreate : text.create}</button>
    <button disabled={listing} onclick={() => list(false)}>{text.refresh}</button>
    <label>{text.checkpoint} <select bind:value={selected} onchange={() => { comparison = null; }}>
      <option value="">{text.select}</option>
      {#each items as item (item.id)}<option value={item.id}>{item.name} ({text.revision}{item.source_revision})</option>{/each}
    </select></label>
    {#if next}<button disabled={listing} onclick={() => list(true)}>{text.more}</button>{/if}
    <label>{text.target} <select bind:value={target} onchange={() => { comparison = null; }}>
      <option value="">{text.current}</option>
      {#each items as item (item.id)}<option value={item.id}>{item.name}</option>{/each}
    </select></label>
    <button disabled={!selected} onclick={() => compare()}>{text.compare}</button>
    <button disabled={!selected || stale || editor.drafting || !!editor.incoming} onclick={restore}>{text.restore}</button>
    <button disabled={!selected || !!pendingDelete} onclick={() => { deleteCandidate = selected; }}>{text.remove}</button>
    {#if pendingDelete}
      <button onclick={() => remove(pendingDelete)}>{text.retryDelete}</button>
    {:else if deleteCandidate}
      <div class="delete-confirmation">
        <p>{text.deletePrompt.replace('{name}', () => items.find(c => c.id === deleteCandidate)?.name ?? '')}</p>
        <div class="delete-actions">
          <button onclick={() => { deleteCandidate = ''; }}>{text.cancel}</button>
          <button onclick={() => remove(deleteCandidate)}>{text.delete}</button>
        </div>
      </div>
    {/if}
  </fieldset>
  {#if listing}<p role="status">{text.loading}</p>{/if}
  {#if editor.drafting}<p>{text.draft}</p>{/if}
  {#if comparison}
    <p>{text.comparisonScope}</p>
    <p role="status">{text.changes.replace('{count}', String(comparison.total_changes))} / {comparison.target_revision !== null ? `${text.targetRevision} ${comparison.target_revision}` : text.fixedComparison}</p>
    <p>{text.duration}: {comparison.before_duration_us / 1_000_000} {text.seconds} → {comparison.after_duration_us / 1_000_000} {text.seconds}</p>
    {#if stale}<p role="alert">{text.stale}</p>{/if}
    <div class="differences">
      {#each comparison.entries as entry}
        <div class="difference">
          <strong>{differenceLabel(entry)}</strong>
          {#if editor.snapshot && entry.entity_id}<p>{differenceSubject(entry, editor.snapshot.project)}</p>{/if}
          <div><span>{text.before}</span><code>{differenceValue(entry.before, entry)}</code></div>
          <div><span>{text.after}</span><code>{differenceValue(entry.after, entry)}</code></div>
        </div>
      {/each}
    </div>
    {#if comparison.next_offset !== null}<button disabled={busy || stale} onclick={() => compare(true)}>{text.moreChanges}</button>{/if}
  {/if}
</details>
<style>
  .checkpoints { margin-top: 20px; padding-top: 20px; border-top: 1px solid var(--border); }
  fieldset { border: 0; padding: 0; display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
  label { display: flex; gap: 8px; align-items: center; font-size: 13px; }
  button { padding: 8px 12px; font-size: 13px; }
  select { max-width: 300px; }
  .delete-confirmation { flex-basis: 100%; border: 1px solid var(--border); border-radius: 8px; padding: 12px; }
  .delete-confirmation p { margin: 0 0 12px; overflow-wrap: anywhere; }
  .delete-actions { display: flex; flex-wrap: wrap; gap: 8px; }
  .difference { border: 1px solid var(--border); padding: 12px; margin: 8px 0; overflow-wrap: anywhere; }
  .difference div { display: grid; grid-template-columns: 60px 1fr; margin-top: 8px; }
  code { white-space: pre-wrap; }
</style>
