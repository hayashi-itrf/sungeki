<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { EditorController } from './controller.svelte';
  let { editor }: { editor: EditorController } = $props();
  let seconds = $state(0);
  async function move(event: SubmitEvent) {
    event.preventDefault();
    const delta = Math.round(seconds * 1e6);
    if (!Number.isSafeInteger(delta)) return;
    await editor.apply([{ type: 'move_selection', scene_ids: [], group_ids: [], clip_ids: [...editor.selection], delta_us: delta }]);
  }
</script>
<form onsubmit={move} oninput={() => { editor.drafting = true; }}>
  <h3>{locale.text('selectionCount').replace('{count}', String(editor.selection.length))}</h3>
  <p>{locale.text('selectionHint')}</p>
  <fieldset disabled={editor.busy || !!editor.pending}>
    <label>{locale.text('selectionOffset')}<input type="number" min="-604800" max="604800" step="0.000001" required bind:value={seconds} /></label>
    <p>{locale.text('selectionMoveHint')}</p>
    <button type="submit">{locale.text('selectionMove')}</button>
    {#if editor.drafting}<button type="button" onclick={() => editor.acceptIncoming()}>{locale.text('inspectorDiscard')}</button>{/if}
    <button type="button" disabled={editor.drafting} onclick={() => { editor.selected = null; }}>{locale.text('selectionClear')}</button>
  </fieldset>
</form>
<style>
  h3 { min-height:52px; display:flex; align-items:center; margin:-14px -14px 14px; padding:11px 14px; background:var(--panel-header); border-bottom:1px solid var(--panel-border); }
  fieldset { border:0; padding:0; }
  label { display:flex; flex-direction:column; gap:6px; }
  input { width:100%; }
  p { font-size:12px; }
  button { margin:4px 0; }
</style>
