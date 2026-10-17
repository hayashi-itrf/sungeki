<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { open, save as chooseSavePath } from '@tauri-apps/plugin-dialog';
  import SessionBrowser from './SessionBrowser.svelte';
  import CollectionPanel from '../collection/CollectionPanel.svelte';
  import BuildJobs from '../production/BuildJobs.svelte';
  import Modal from '../../shared/components/Modal.svelte';
  import { tick } from 'svelte';
  import type { SessionRequest } from '@sungeki/api-client';
  import EditorNotices from './EditorNotices.svelte';
  import type { EditorController } from './controller.svelte';
  import { handleShortcut } from './shortcuts';
  let { editor, onopensettings }: { editor: EditorController; onopensettings: () => void } = $props();
  let filesOpen = $state(false);
  let title = $state('');
  let path = $state('');
  let choosing = $state(false);
  let importCheckpoints = $state(false);
  const unavailable = $derived(editor.busy || choosing || !!editor.pending);
  const filters = $derived([{ name: locale.text('projectFilter'), extensions: ['json'] }]);
  function keydown(event: KeyboardEvent) {
    const editingText = event.composedPath().some(target => target instanceof HTMLElement &&
      (target.isContentEditable || target.matches('input, textarea, select, [role="textbox"]')));
    handleShortcut(event, editor, asNew => { void chooseSave(asNew); }, {
      editingText, modalOpen: !!document.querySelector('dialog[open]'), choosing,
    });
  }
  async function chooseOpen() {
    if (unavailable || editor.drafting) return;
    choosing = true;
    const includeHistory = importCheckpoints;
    try {
      const selected = await open({ title: locale.text('openPrompt'), multiple: false, directory: false, filters });
      if (selected) { path = selected; await activate({ type: 'open', path: selected, import_checkpoints: includeHistory }); }
    } catch (error) { editor.error = String(error); }
    finally { choosing = false; }
  }
  async function chooseSave(asNew: boolean) {
    const current = editor.snapshot;
    if (!current || unavailable || editor.drafting) return;
    choosing = true;
    try {
      const selected = !asNew && current.document_path ? current.document_path : await chooseSavePath({
        title: locale.text('saveProject'), filters,
        defaultPath: current.document_path ?? `${current.project.title.replace(/[\\/:*?"<>|]/g, '_') || locale.text('newProject')}.vvm.json`,
      });
      if (selected) {
        path = selected;
        await editor.request({ type: 'save', session_id: current.session_id, path: selected, expected_revision: current.revision });
      }
    } catch (error) { editor.error = String(error); }
    finally { choosing = false; }
  }
  async function activate(request: SessionRequest) {
    filesOpen = false;
    await tick();
    await editor.request(request);
    if (editor.error) filesOpen = true;
  }
  async function save(asNew: boolean) {
    const current = editor.snapshot;
    if (!current) return;
    await editor.request({ type: 'save', session_id: current.session_id,
      path: asNew || !current.document_path ? path : null, expected_revision: current.revision });
  }
</script>
<svelte:window onkeydown={keydown} />
<div class="toolbar">
  <BuildJobs {editor} />
  {#if editor.snapshot}{#key editor.snapshot.session_id}<CollectionPanel {editor} />{/key}{/if}
  <button onclick={() => {
    importCheckpoints = false;
    title = editor.snapshot?.project.title ?? locale.text('newProject');
    path = editor.snapshot?.document_path ?? '';
    filesOpen = true;
  }}>{locale.text('files')}</button>
  <strong class="project-name">{editor.snapshot?.project.title ?? locale.text('openPrompt')}</strong>
  {#if editor.snapshot}
    <small class="saved">{editor.drafting ? locale.text('draft') : editor.snapshot.dirty ? locale.text('dirty') : locale.text('saved')}</small>
    <button title={locale.text('saveProject') + ' (⌘/Ctrl+S)'} disabled={unavailable || editor.drafting} onclick={() => chooseSave(false)}>{locale.text('saveProject')}</button>
    <button title={locale.text('undo') + ' (⌘/Ctrl+Z)'} disabled={unavailable || editor.drafting || !editor.snapshot.undo_operation_id} onclick={() => editor.history('undo')}>{locale.text('undo')}</button>
    <button title={locale.text('redo') + ' (⌘/Ctrl+Shift+Z)'} disabled={unavailable || editor.drafting || !editor.snapshot.redo_operation_id} onclick={() => editor.history('redo')}>{locale.text('redo')}</button>
  {/if}
  <button class="settings" onclick={onopensettings}>{locale.text('settings')}</button>
</div>
<Modal bind:open={filesOpen} title={locale.text('files')}>
  <p>{locale.text('moveProjectHint')}</p>
  <EditorNotices {editor} />
  {#if editor.drafting}<p role="status">{locale.text('draftFileHint')}</p>{/if}
  <fieldset disabled={unavailable || editor.drafting}>
    <legend>{locale.text('project')}</legend>
    <label>{locale.text('projectName')} <input bind:value={title} /></label>
    <button onclick={() => activate({ type: 'create', title })}>{locale.text('create')}</button>
    {#if editor.snapshot}<button onclick={() => editor.apply([{ type: 'rename_project', title }])}>{locale.text('rename')}</button>{/if}
  </fieldset>
  <fieldset disabled={unavailable || editor.drafting}>
    <legend>{locale.text('file')}</legend>
    <label><input type="checkbox" bind:checked={importCheckpoints} /> {locale.text('importCheckpoints')}</label>
    <small>{locale.text('importCheckpointHint')}</small>
    <button onclick={chooseOpen}>{locale.text('chooseOpen')}</button>
    <button disabled={!editor.snapshot} onclick={() => chooseSave(false)}>{locale.text('saveProject')}</button>
    <button disabled={!editor.snapshot} onclick={() => chooseSave(true)}>{locale.text('saveAs')}</button>
    <details><summary>{locale.text('specifyPath')}</summary>
    <label>{locale.text('absolutePath')} <input placeholder="/…/project.vvm.json" bind:value={path} /></label>
    <button disabled={!path.trim()} onclick={() => activate({ type: 'open', path, import_checkpoints: importCheckpoints })}>{locale.text('open')}</button>
    <button disabled={!editor.snapshot || (!editor.snapshot.document_path && !path.trim())} onclick={() => save(false)}>{locale.text('saveProject')}</button>
    <button disabled={!editor.snapshot || !path.trim()} onclick={() => save(true)}>{locale.text('saveAs')}</button>
    </details>
    {#if editor.snapshot?.document_path}<small>{locale.text('savedPath')}{editor.snapshot.document_path}</small>{/if}
  </fieldset>
  <SessionBrowser disabled={unavailable || editor.drafting} current={editor.snapshot} {activate} />
  {#if editor.snapshot}<button disabled={editor.busy} onclick={() => editor.request({ type: 'get', session_id: editor.snapshot!.session_id })}>{locale.text('refresh')}</button>{/if}
</Modal>
<style>
  .toolbar { background:var(--surface); display:flex; align-items:center; gap:8px; padding:8px 14px; border-bottom:1px solid var(--border); }
  .project-name { flex:1; min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .saved { width:auto; white-space:nowrap; color:var(--muted); }
  fieldset { border: 0; padding: 8px 0; display: flex; flex-wrap: wrap; align-items: center; gap: 8px; min-width: 0; }
  legend { font-weight: 600; }
  small { width: 100%; overflow-wrap: anywhere; }
  label { display: flex; align-items: center; gap: 8px; font-size: 13px; }
  input:not([type="checkbox"]) { min-width: 200px; }
  details { width:100%; }
  details label { margin:10px 0; }
  .toolbar > button { flex-shrink:0; white-space:nowrap; }
  button { padding: 8px 12px; font-size: 13px; }
</style>
