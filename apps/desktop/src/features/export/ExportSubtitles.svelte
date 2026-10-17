<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { onDestroy } from 'svelte';
  import { save } from '@tauri-apps/plugin-dialog';
  import type { SubtitleFormat } from '@sungeki/api-client';
  import type { EditorController } from '../editor/controller.svelte';
  import { hostApi } from '../../shared/host-api';
  import Modal from '../../shared/components/Modal.svelte';
  import { SubtitlesController } from './subtitles.svelte';
  let { editor }: { editor: EditorController } = $props();
  const output = (() => new SubtitlesController(hostApi, editor.snapshot!.session_id))();
  let open = $state(false), format = $state<SubtitleFormat>('srt'), includeText = $state(false);
  const disabled = $derived(output.busy || !!output.pending || editor.busy || editor.drafting || !!editor.pending);
  const stale = $derived(output.result && (output.result.stale
    || output.result.source_revision !== editor.snapshot?.revision));
  onDestroy(() => output.dispose());
  function choose() {
    const snapshot = editor.snapshot;
    if (!snapshot || disabled) return;
    const extension = format === 'srt' ? 'srt' : 'vtt';
    return output.choose({ session_id: snapshot.session_id, expected_revision: snapshot.revision,
      format, include_text: includeText }, () => save({ title: locale.text('subtitleSaveDialog'),
      defaultPath: `${snapshot.project.title.replace(/[\\/:*?"<>|]/g, '_') || locale.text('subtitleFilename')}.${extension}`,
      filters: [{ name: format === 'srt' ? locale.text('subtitleSrt') : locale.text('subtitleVtt'), extensions: [extension] }] }));
  }
</script>
<button disabled={editor.busy || editor.drafting || !!editor.pending} onclick={() => open = true}>{locale.text('subtitleExport')}</button>
<Modal bind:open title={locale.text('subtitleExport')}>
  <p>{locale.text('subtitleIntro')}</p>
  <fieldset {disabled}>
    <label>{locale.text('subtitleFormat')}<select bind:value={format}><option value="srt">SRT（.srt）</option><option value="web_vtt">WebVTT（.vtt）</option></select></label>
    <label><input type="checkbox" bind:checked={includeText} />{locale.text('subtitleIncludeText')}</label>
    <button onclick={choose}>{locale.text('subtitleChooseExport')}</button>
  </fieldset>
  <p>{locale.text('subtitleScope')}</p>
  {#if output.busy}<p role="status">{locale.text('subtitleBusy')}</p>{/if}
  {#if output.error}<p role="alert">{output.error}</p>{/if}
  {#if output.pending && !output.busy}
    <p role="alert">{locale.text('subtitleUncertain')}</p>
    <label>{locale.text('subtitlePendingPath')}<input readonly value={output.pending.path} onclick={e => e.currentTarget.select()} /></label>
    <button onclick={() => output.retry()}>{locale.text('subtitleRetry')}</button>
    <p>{locale.text('subtitleRetryHint')}</p>
    <button onclick={() => output.dismiss()}>{locale.text('subtitleDismiss')}</button>
    <p>{locale.text('subtitleDismissHint')}</p>
  {/if}
  {#if output.result}
    <p role="status">{output.result.already_present ? locale.text('subtitleExisting') : locale.text('subtitleExported')}</p>
    <label>{locale.text('subtitleSavedPath')}<input readonly value={output.result.path} onclick={e => e.currentTarget.select()} /></label>
    {#if stale}<p role="alert">{locale.text('subtitleStale')}</p>{/if}
  {/if}
</Modal>
<style>
  fieldset { display:flex; flex-direction:column; gap:12px; border:1px solid var(--panel-border); border-radius:6px; padding:14px; }
  label { display:flex; align-items:center; gap:8px; flex-wrap:wrap; }
  input[readonly] { width:100%; }
  p { line-height:1.6; }
</style>
