<script lang="ts">
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { scriptText as t } from './script-messages';
  import type { EditorController } from '../editor/controller.svelte';
  import EditorNotices from '../editor/EditorNotices.svelte';
  import ScriptForm from './ScriptForm.svelte';
  let { editor }: { editor: EditorController } = $props();
  let open = $state(false);
  let submittedId = $state('');
  let dialog = $state<HTMLDialogElement>();
  $effect(() => { if (open && dialog) dialog.showModal(); });
  $effect(() => {
    // Includes successful replay through EditorNotices after an uncertain result.
    if (open && submittedId && !editor.busy && !editor.pending && !editor.drafting
      && editor.snapshot?.project.clips.some(c => c.id === submittedId)) {
      editor.revealClip(submittedId); submittedId = ''; open = false;
    }
  });
  function close() {
    if (editor.busy || editor.pending) return;
    editor.acceptIncoming(); submittedId = ''; open = false;
  }
</script>
<button disabled={!editor.snapshot || editor.busy || editor.drafting || !!editor.pending} onclick={() => { submittedId = ''; open = true; }}>{t("＋ 台本")}</button>
{#if open && editor.snapshot}
  <dialog use:containDialogFocus bind:this={dialog} aria-label={t("台本からセリフを追加")} oncancel={e => { e.preventDefault(); close(); }}>
    <header><h2>{t("台本からセリフを追加")}</h2><button disabled={editor.busy || !!editor.pending} onclick={close}>{editor.drafting ? t("入力を取り消して閉じる") : t("閉じる")}</button></header>
    <EditorNotices {editor} />
    {#key `${editor.snapshot.session_id}:${editor.snapshot.revision}:${editor.draftGeneration}`}
      <ScriptForm {editor} submitting={id => { submittedId = id; }} />
    {/key}
  </dialog>
{/if}
<style>header { display:flex; justify-content:space-between; align-items:center; gap:12px; }</style>
