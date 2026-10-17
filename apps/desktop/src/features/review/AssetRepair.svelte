<script lang="ts">
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { locale } from '../../shared/i18n/locale.svelte';
  import AssetDetails from '../editor/AssetDetails.svelte';
  import EditorNotices from '../editor/EditorNotices.svelte';
  import type { EditorController } from '../editor/controller.svelte';
  let { editor, assetId, onclose }: { editor: EditorController; assetId: string; onclose: () => void } = $props();
  let dialog = $state<HTMLDialogElement>();
  $effect(() => { dialog?.showModal(); });
  const locked = $derived(editor.busy || !!editor.pending || editor.drafting);
</script>
<dialog use:containDialogFocus bind:this={dialog} aria-label={locale.text('reviewRepairTitle')} oncancel={event => { event.preventDefault(); if (!locked) onclose(); }}>
  <header><h2>{locale.text('reviewTargetRepair')}</h2><button disabled={locked} onclick={onclose}>{locale.text('reviewRepairClose')}</button></header>
  <EditorNotices {editor} />
  {#if editor.snapshot?.project.assets.some(asset => asset.id === assetId)}
    {#key `${editor.snapshot.session_id}:${editor.snapshot.revision}:${assetId}`}<AssetDetails {editor} {assetId} />{/key}
    <p>{locale.text('reviewRepairAgain')}</p>
  {:else}<p>{locale.text('reviewRepairMissing')}</p>{/if}
</dialog>
<style>header { display:flex; align-items:center; justify-content:space-between; gap:12px; }</style>
