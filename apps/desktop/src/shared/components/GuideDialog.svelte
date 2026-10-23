<script lang="ts">
  import type { Snippet } from 'svelte';
  import { containDialogFocus } from './dialog-focus';
  import { locale } from '../i18n/locale.svelte';
  let { open = $bindable(false), title, items, selected = $bindable(''), children }: {
    open?: boolean; title: string; items: { id: string; label: string }[];
    selected?: string; children: Snippet;
  } = $props();
  let dialog: HTMLDialogElement;
  let content: HTMLDivElement;
  $effect(() => { selected; if (content) content.scrollTop = 0; });
  $effect(() => { if (!dialog) return; if (open && !dialog.open) dialog.showModal(); else if (!open && dialog.open) dialog.close(); });
</script>
<dialog bind:this={dialog} use:containDialogFocus aria-label={title} onclose={() => open = false}>
  <header><h1>{title}</h1><button onclick={() => open = false}>{locale.text('close')}</button></header>
  <div class="layout">
    <nav aria-label={title}>
      {#each items as item}
        <button class:active={selected === item.id} aria-current={selected === item.id ? 'page' : undefined} onclick={() => selected = item.id}>{item.label}</button>
      {/each}
    </nav>
    <div class="content" bind:this={content}>{@render children()}</div>
  </div>
</dialog>
<style>
  dialog { width:min(1040px,94vw); height:min(760px,88dvh); max-height:88dvh; padding:0; overflow:hidden; }
  header { display:flex; align-items:center; justify-content:space-between; padding:14px 20px; border-bottom:1px solid var(--border); }
  h1 { margin:0; font-size:20px; }
  .layout { display:grid; grid-template-columns:200px minmax(0,1fr); height:calc(100% - 63px); min-height:0; }
  nav { padding:12px; border-right:1px solid var(--border); overflow:auto; background:var(--background); }
  nav button { display:block; width:100%; text-align:left; padding:11px 12px; margin-bottom:6px; }
  nav button.active { border-color:var(--accent,#315fff); background:var(--panel-header); color:var(--text); box-shadow:inset 3px 0 var(--accent,#315fff); font-weight:600; }
  .content { overflow:auto; min-width:0; padding:20px 24px; line-height:1.7; }
  .content :global([hidden]) { display:none !important; }
  .content :global(h2:first-child) { margin-top:0; }
  @media(max-width:600px) { .layout { grid-template-columns:140px minmax(0,1fr); } nav { padding:8px; } .content { padding:14px; } }
</style>
