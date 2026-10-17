<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { MessageKey } from '../../shared/i18n/messages';
  import { onMount } from 'svelte';
  import type { Command } from '@sungeki/api-client';
  import { chooseReplacement, probeAsset } from './asset-probe';
  import { assetUses } from './asset-uses';
  import { queueImage } from './image-preview-queue';
  import type { EditorController } from './controller.svelte';
  const assetError = (error: unknown) => error && typeof error === 'object' && 'message' in error ? String(error.message) : String(error);
  let { editor, assetId }: { editor: EditorController; assetId: string } = $props();
  const asset = $derived(editor.snapshot?.project.assets.find(a => a.id === assetId));
  const uses = $derived(editor.snapshot ? assetUses(editor.snapshot.project, assetId).length : 0);
  let checking = $state(false); let statusKey = $state<MessageKey>('assetUnchecked'); let statusError = $state('');
  const status = $derived(statusError || locale.text(statusKey)); let failed = $state(false);
  let active = true;
  onMount(() => { void inspect(); return () => { active = false; }; });
  async function inspect() {
    const target = asset;
    if (!target || checking) return;
    checking = true; statusKey = 'assetChecking'; statusError = ''; failed = false;
    await queueImage(async () => {
      if (!active) return;
      try {
        if (target.locator.type !== 'absolute') { failed = true; statusKey = 'assetUnsupported'; return; }
        const result = await probeAsset(target.locator.path, target.probe);
        if (!active) return;
        failed = result.info.sha256 !== target.content_hash;
        statusKey = failed ? 'assetChanged' : 'assetMatching';
      } catch (error) { if (active) { failed = true; statusError = assetError(error); } }
      finally { if (active) checking = false; }
    });
  }
  async function relink() {
    const current = editor.snapshot; const target = asset;
    if (!current || !target || editor.busy || editor.pending || editor.drafting || checking) return;
    editor.busy = true; editor.error = '';
    let command: Command | null = null;
    try {
      const chosen = await chooseReplacement(target.probe);
      if (chosen) command = { type: 'relink_asset', asset_id: target.id, locator: { type: 'absolute', path: chosen.path }, probe: chosen.probe };
    } catch (error) { editor.error = assetError(error); }
    finally {
      editor.busy = false;
      if (!command && editor.incoming && !editor.drafting && !editor.pending) {
        const error = editor.error; editor.acceptIncoming(); editor.error = error;
      }
    }
    if (command) await editor.request({ type: 'edit', session_id: current.session_id, edit: {
      operation_id: crypto.randomUUID(), expected_revision: current.revision, action: { type: 'apply', commands: [command] } } });
  }
</script>
{#if asset}
  <section aria-label={locale.text('assetLabel')} class="asset">
    <strong>{asset.name}</strong>
    <details><summary>{locale.text('assetLocation')}</summary><small>{asset.locator.path}</small></details>
    {#if asset.probe.type === 'image'}<small>{locale.text('assetRegistered')}{asset.probe.info.width} × {asset.probe.info.height} ・ {asset.probe.info.format.toUpperCase()}</small>{/if}
    {#if asset.probe.type === 'audio'}<small>{locale.text('assetSpeechOriginal')}</small>{/if}
    <p role="status" class:failed>{status}</p>
    <div class="actions">
      <button type="button" disabled={checking || editor.busy} onclick={inspect}>{locale.text('assetCheck')}</button>
      <button type="button" disabled={checking || editor.busy || !!editor.pending || editor.drafting} onclick={relink}>{locale.text('assetRelink')}</button>
    </div>
    <small>{locale.text('assetUses').replace('{count}', String(uses))}</small>
    {#if editor.drafting}<small>{locale.text('assetDraftHint')}</small>{/if}
    {#if asset.credit}<small>{locale.text('assetCreditPrefix')}{asset.credit}</small>{/if}
  </section>
{/if}
<style>
  .asset { border:1px solid var(--panel-border); background:var(--background); border-radius:5px; padding:10px; margin-bottom:12px; }
  details { margin:6px 0; font-size:11px; }
  strong,small { display:block; overflow-wrap:anywhere; }
  small { color:var(--muted); margin:6px 0; font-size:11px; }
  p { margin:8px 0; font-size:12px; }
  .failed { border-left:3px solid var(--accent); padding-left:6px; }
  .actions { display:flex; flex-wrap:wrap; gap:4px; }
  button { font-size:12px; padding:5px 7px; }
</style>
