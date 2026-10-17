<script lang="ts">
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { locale } from '../../shared/i18n/locale.svelte';

  import OutputPlayback from './OutputPlayback.svelte';
  import type { RenderJob } from '@sungeki/api-client';
  let { job, initiallyOpen = false }: { job: RenderJob; initiallyOpen?: boolean } = $props();
  let open = $state((() => initiallyOpen)()), reload = $state(0);
  let dialog = $state<HTMLDialogElement>();
  const title = $derived(locale.text(job.purpose === 'editing_preview' ? 'playerPreview' : 'playerFinal'));
  $effect(() => {
    if (!open || !dialog) return;
    const current = dialog;
    current.showModal();
    return () => current.close();
  });
</script>
<button onclick={() => { open = true; }}>{title}</button>
{#if open}
  <dialog use:containDialogFocus bind:this={dialog} aria-label={title} onclose={() => { open = false; }}>
  <header>
    <h2>{title}</h2>
    <button onclick={() => { open = false; }}>{locale.text('playerClose')}</button>
  </header>
  <div class="details">
  {#if job.purpose === 'editing_preview'}
    <p>{locale.text('playerRevision')} {job.source_revision}: {locale.text('playerPreviewHint')}</p>
  {:else}
    <p>{locale.text('playerRevision')} {job.source_revision}: {locale.text('playerFinalHint')}</p>
  {/if}
  <button onclick={() => { reload++; }}>{locale.text('playerReload')}</button>
  </div>
  {#key `${job.job_id}:${reload}`}
    <OutputPlayback jobId={job.job_id} />
  {/key}
  </dialog>
{/if}
<style>
  dialog { box-sizing:border-box; width:min(1100px,calc(100vw - 32px)); height:calc(100dvh - 32px); max-height:none; padding:16px; overflow:hidden; }
  dialog[open] { display:flex; flex-direction:column; gap:10px; }
  header, .details { display:flex; align-items:center; justify-content:space-between; gap:12px; flex-shrink:0; }
  h2, p { margin:0; }
  h2 { font-size:18px; }
  .details { font-size:13px; }
  button { flex-shrink:0; }
  @media (max-width:600px) { .details { align-items:flex-start; flex-wrap:wrap; } }
</style>
