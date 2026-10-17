<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { onMount } from 'svelte';
  import type { SessionRequest, SessionSnapshot } from '@sungeki/api-client';
  import { hostApi } from '../../shared/host-api';
  import { SessionBrowserModel } from './session-browser.svelte';
  let { disabled, current, activate }: { disabled: boolean; current: SessionSnapshot | null; activate: (request: SessionRequest) => Promise<void> } = $props();
  const model = new SessionBrowserModel(hostApi);
  onMount(() => { void model.refresh(); return () => model.dispose(); });
</script>
<section aria-label={locale.text('recoverableProjects')}>
  <div class="heading"><h3>{locale.text('recoverableProjects')}</h3><button disabled={model.busy || disabled} onclick={() => model.refresh()}>{locale.text('refreshList')}</button></div>
  <p>{locale.text('resumeHint')}</p>
  {#if model.error}<p role="alert">{model.error}</p>{/if}
  <div class="entries" aria-busy={model.busy}>
    {#each model.entries as entry (entry.id)}
      {@const active = current?.session_id === entry.id ? current : null}
      {#if entry.summary}
        <button class="entry" class:current={!!active} disabled={disabled || !!active}
          onclick={() => activate({ type: 'get', session_id: entry.id })}>
          <span class="name">{(active ? active.project.title : entry.summary.title) || locale.text('untitled')}</span>
          <span class="state">{active ? locale.text('editing') + ' · ' : ''}{(active ? active.dirty : entry.summary.dirty) ? locale.text('dirty') : locale.text('saved')}</span>
          <span class="path">{(active ? active.document_path : entry.summary.document_path) ?? locale.text('noSavedPath')}</span>
          <small>{active ? active.project.clips.length : entry.summary.counts.clips} {locale.text('clips')} · {locale.text('revision')} {active ? active.revision : entry.revision} · {locale.text('identifier')} {entry.id.slice(0, 8)}</small>
        </button>
      {:else}
        <div class="failed"><strong>{locale.text('projectReadFailed')}</strong><p>{entry.error}</p><small>{locale.text('identifier')} {entry.id.slice(0,8)} · {locale.text('retryListHint')}</small></div>
      {/if}
    {/each}
  </div>
  {#if model.busy}<p role="status">{locale.text('loadingProjects')}</p>{/if}
  {#if model.loaded && !model.busy && !model.ids.length}<p>{locale.text('noProjects')}</p>{/if}
  {#if model.entries.length < model.ids.length}<button disabled={model.busy || disabled} onclick={() => model.more()}>{locale.text('showMore')} ({model.entries.length} / {model.ids.length})</button>{/if}
</section>
<style>
  section { border-top:1px solid var(--border); margin-top:16px; padding-top:14px; }
  .heading { display:flex; align-items:center; justify-content:space-between; gap:10px; }
  h3 { margin:0; } p { font-size:12px; }
  .entries { display:grid; gap:8px; max-height:320px; overflow:auto; }
  .entry { text-align:left; display:grid; grid-template-columns:1fr auto; gap:6px 12px; padding:12px; }
  .entry .name { font-weight:700; overflow-wrap:anywhere; } .state { font-size:11px; color:var(--muted); }
  .path,small { grid-column:1 / -1; overflow-wrap:anywhere; color:var(--muted); font-size:11px; }
  .current { border-color:var(--accent); opacity:1; background:var(--panel-header); }
  .failed { border:1px solid var(--border); padding:12px; }
</style>
