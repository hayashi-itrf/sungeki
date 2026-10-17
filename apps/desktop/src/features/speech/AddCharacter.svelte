<script lang="ts">
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { characterText as t } from './character-messages';
  import type { EditorController } from '../editor/controller.svelte';
  import EditorNotices from '../editor/EditorNotices.svelte';
  import CharacterForm from './CharacterForm.svelte';
  let { editor }: { editor: EditorController } = $props();
  let open = $state(false), submittedId = $state('');
  let dialog = $state<HTMLDialogElement>();
  $effect(() => { if (open && dialog) dialog.showModal(); });
  $effect(() => {
    if (open && submittedId && !editor.busy && !editor.pending && !editor.drafting
      && editor.snapshot?.project.characters.some(c => c.id === submittedId)) { submittedId = ''; open = false; }
  });
  function close() {
    if (editor.busy || editor.pending) return;
    editor.acceptIncoming(); submittedId = ''; open = false;
  }
</script>
<button disabled={!editor.snapshot || editor.busy || editor.drafting || !!editor.pending} onclick={() => { submittedId = ''; open = true; }}>{t("＋ キャラクター")}</button>
{#if open && editor.snapshot}
  <dialog use:containDialogFocus bind:this={dialog} aria-label={t("キャラクターを登録")} oncancel={e => { e.preventDefault(); close(); }}>
    <header><h2>{t("キャラクターを登録")}</h2><button disabled={editor.busy || !!editor.pending} onclick={close}>{editor.drafting ? t("入力を取り消して閉じる") : t("閉じる")}</button></header>
    <EditorNotices {editor} />
    {#key `${editor.snapshot.session_id}:${editor.snapshot.revision}:${editor.draftGeneration}`}<CharacterForm {editor} submitting={id => { submittedId = id; }} />{/key}
  </dialog>
{/if}
<style>header { display:flex; justify-content:space-between; align-items:center; gap:12px; }</style>
