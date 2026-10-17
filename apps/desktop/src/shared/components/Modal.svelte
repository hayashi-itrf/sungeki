<script lang="ts">
  import { locale } from '../i18n/locale.svelte';
  import { containDialogFocus } from './dialog-focus';
  import type { Snippet } from 'svelte';
  let { open = $bindable(false), title, children, stickyHeader = false }: { open: boolean; title: string; children: Snippet; stickyHeader?: boolean } = $props();
  let element = $state<HTMLDialogElement>();
  $effect(() => {
    if (open && element) {
      const current = element;
      current.showModal();
      return () => current.close();
    }
  });
</script>
{#if open}
  <dialog bind:this={element} use:containDialogFocus aria-label={title} onclose={() => { open = false; }}>
    <div class="heading" class:sticky={stickyHeader}><h2>{title}</h2><button onclick={() => { open = false; }}>{locale.text('close')}</button></div>
    {@render children()}
  </dialog>
{/if}
<style>
  .heading { display:flex; align-items:center; justify-content:space-between; gap:12px; }
  .heading.sticky { position:sticky; top:-20px; z-index:2; background:var(--surface); padding:8px 0; border-bottom:1px solid var(--panel-border); }
</style>
