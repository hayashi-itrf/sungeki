<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke, isTauri } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import type { ExitStatus } from '@sungeki/api-client';
  import type { EditorController } from './controller.svelte';
  import { locale } from '../../shared/i18n/locale.svelte';
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  let { editor }: { editor: EditorController } = $props();
  let dialog: HTMLDialogElement;
  let checking = $state(false);
  let finishing = $state(false);
  let status = $state<ExitStatus | null>(null);
  let error = $state('');
  let cleanup = $state(false);
  const unsaved = $derived(!!editor.snapshot?.dirty || editor.drafting || !!editor.pending);
  function message(e: unknown) { return e && typeof e === 'object' && 'message' in e ? String(e.message) : String(e); }
  async function inspect() {
    if (checking || finishing || dialog.open) return;
    checking = true; error = ''; status = null;
    try {
      status = await invoke<ExitStatus>('exit_status');
      const anotherDialog = !!document.querySelector('dialog[open]');
      if (!anotherDialog && !unsaved && !editor.busy && !status.active_jobs && !status.other_clients && !status.pending_requests) {
        await finish(false);
      } else { dialog.showModal(); }
    } catch (e) { error = message(e); dialog.showModal(); }
    finally { checking = false; }
  }
  async function finish(background: boolean) {
    if (finishing) return;
    finishing = true; error = '';
    try { await invoke('finish_exit', { background, cancelJobs: !!status?.active_jobs }); }
    catch (e) {
      error = message(e);
      if (e && typeof e === 'object' && 'details' in e) cleanup = (e.details as Record<string, string>).exit_stopping === 'true';
      if (!dialog.open) dialog.showModal();
    }
    finally { finishing = false; }
  }
  onMount(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen('app-quit-requested', () => { void inspect(); }).then(async stop => {
      if (disposed) { stop(); return; }
      unlisten = stop;
      await invoke('exit_ready');
    });
    return () => { disposed = true; unlisten?.(); };
  });
</script>
<dialog bind:this={dialog} use:containDialogFocus aria-label={locale.text('quitTitle')}
  oncancel={event => { if (finishing || cleanup) event.preventDefault(); }}>
  <h2>{locale.text('quitTitle')}</h2>
  {#if unsaved}<p>{locale.text('quitUnsaved')}</p>{/if}
  {#if status?.active_jobs}<p>{locale.text('quitJobs')}</p>{/if}
  {#if status?.other_clients}<p>{locale.text('quitOthers')}</p>{/if}
  {#if status?.pending_requests || editor.busy}<p>{locale.text('quitPending')}</p>{/if}
  {#if !status?.active_jobs && !status?.other_clients && !status?.pending_requests}<p>{locale.text('quitConfirm')}</p>{/if}
  {#if finishing}<p role="status">{locale.text('quitWaiting')}</p>{/if}
  {#if error}<p role="alert">{error}</p>{/if}
  <div class="actions">
    <button disabled={finishing || cleanup} onclick={() => dialog.close()}>{locale.text('quitReturn')}</button>
    <button disabled={finishing} onclick={() => finish(true)}>{locale.text('quitBackground')}</button>
    <button disabled={finishing || !status || !!status.other_clients} onclick={() => finish(false)}>
      {status?.active_jobs ? locale.text('quitCancelJobs') : locale.text('quitExit')}
    </button>
  </div>
  {#if error}<button disabled={finishing} onclick={() => { dialog.close(); void inspect(); }}>{locale.text('quitRetry')}</button>{/if}
</dialog>
<style>
  .actions { display:flex; flex-wrap:wrap; gap:8px; margin-top:20px; }
  p { max-width:540px; line-height:1.7; }
</style>
