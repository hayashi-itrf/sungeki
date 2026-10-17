<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { hostApi } from '../../shared/host-api';
  import { importAudio } from './import-audio';
  import type { Command } from '@sungeki/api-client';
  import type { EditorController } from '../editor/controller.svelte';
  let { editor }: { editor: EditorController } = $props();
  async function add() {
    const current = editor.snapshot;
    if (!current || editor.busy || editor.pending || editor.drafting) return;
    const start = Math.round(editor.time * 1_000_000);
    editor.busy = true; editor.error = '';
    let commands: Command[] | null = null;
    const clip = crypto.randomUUID();
    try {
      const path = await open({ title: locale.text('audioAdd'), multiple: false, directory: false,
        filters: [{ name: locale.text('audioFilter'), extensions: ['wav', 'mp3'] }] });
      if (!path) return;
      const result = await hostApi.session({ type: 'probe_audio', path });
      if (result.type !== 'audio_probed') throw new Error(locale.text('audioProbeFailed'));
      commands = importAudio(path, result.probe, start, { asset: crypto.randomUUID(), track: crypto.randomUUID(), clip });
    } catch (error) {
      editor.error = error && typeof error === 'object' && 'message' in error ? String(error.message) : String(error);
    } finally {
      editor.busy = false;
      if (!commands && editor.incoming && !editor.drafting && !editor.pending) {
        const error = editor.error; editor.acceptIncoming(); editor.error = error;
      }
    }
    if (commands) {
      await editor.request({ type: 'edit', session_id: current.session_id, edit: {
        operation_id: crypto.randomUUID(), expected_revision: current.revision, action: { type: 'apply', commands } } });
      if (!editor.error) editor.selected = clip;
    }
  }
</script>
<button disabled={!editor.snapshot || editor.busy || !!editor.pending || editor.drafting} onclick={add}>{locale.text('audioAddButton')}</button>
