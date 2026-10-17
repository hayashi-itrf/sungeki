<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { hostApi } from '../../shared/host-api';
  import { importVrm } from './import-vrm';
  import type { EditorController } from '../editor/controller.svelte';
  import type { Command } from '@sungeki/api-client';
  let { editor, submitting }: { editor: EditorController; submitting: (id: string) => void } = $props();
  const current = (() => editor.snapshot!)();
  let model = $state(''), motion = $state(''), modelCredit = $state(''), motionCredit = $state('');
  let start = $state((() => editor.time)()), duration = $state(5), sceneId = $state(''), characterId = $state('');
  let choosing = $state(false);
  async function choose(kind: 'model' | 'motion') {
    if (choosing || editor.busy || editor.pending) return;
    choosing = true; editor.busy = true;
    try {
      const path = await open({ title: kind === 'model' ? locale.text('standingChooseModel') : locale.text('standingChooseMotion'),
        multiple: false, directory: false, filters: [{ name: kind === 'model' ? 'VRM' : 'VRMA', extensions: [kind === 'model' ? 'vrm' : 'vrma'] }] });
      if (path) { if (kind === 'model') model = path; else motion = path; editor.drafting = true; }
    } catch (error) { editor.error = String(error); }
    finally { choosing = false; editor.busy = false; }
  }
  async function add(event: SubmitEvent) {
    event.preventDefault();
    if (editor.busy || editor.pending || choosing || !model || !motion) return;
    editor.busy = true; editor.error = '';
    let commands: Command[] | undefined;
    const id = crypto.randomUUID();
    try {
      const m = await hostApi.session({ type: 'probe_vrm', path: model, kind: 'model' });
      const a = await hostApi.session({ type: 'probe_vrm', path: motion, kind: 'motion' });
      if (m.type !== 'vrm_probed' || a.type !== 'vrm_probed') throw Error(locale.text('standingVrmProbeError'));
      commands = importVrm(current.project, { path: model, credit: modelCredit, probe: m.probe },
        { path: motion, credit: motionCredit, probe: a.probe }, start, duration, sceneId, characterId,
        { model: crypto.randomUUID(), motion: crypto.randomUUID(), track: crypto.randomUUID(), clip: id });
    } catch (error) { editor.error = error && typeof error === 'object' && 'message' in error ? String(error.message) : String(error); }
    finally { editor.busy = false; }
    if (commands) {
      submitting(id);
      await editor.request({ type: 'edit', session_id: current.session_id, edit: {
        operation_id: crypto.randomUUID(), expected_revision: current.revision, action: { type: 'apply', commands } } });
    }
  }
</script>
<form onsubmit={add} oninput={() => { editor.drafting = true; }}>
  <fieldset disabled={editor.busy || !!editor.pending || choosing}>
    <button type="button" onclick={() => choose('model')}>{locale.text('standingChooseModel')}</button><p>{model || locale.text('standingUnselected')}</p>
    <label>{locale.text('standingModelCredit')}<textarea required bind:value={modelCredit} rows="2"></textarea></label>
    <button type="button" onclick={() => choose('motion')}>{locale.text('standingChooseMotion')}</button><p>{motion || locale.text('standingUnselected')}</p>
    <label>{locale.text('standingMotionCredit')}<textarea required bind:value={motionCredit} rows="2"></textarea></label>
    <label>{locale.text('standingStart')}<input type="number" min="0" max="604800" step="0.000001" required bind:value={start} /></label>
    <label>{locale.text('standingDuration')}<input type="number" min="0.000001" max="7200" step="0.000001" required bind:value={duration} /></label>
    <label>{locale.text('standingScene')}<select bind:value={sceneId}><option value="">{locale.text('standingNoScene')}</option>{#each current.project.scenes as scene}<option value={scene.id}>{scene.name}</option>{/each}</select></label>
    <label>{locale.text('standingCharacter')}<select bind:value={characterId}><option value="">{locale.text('standingNoCharacter')}</option>{#each current.project.characters as c}<option value={c.id}>{c.name}</option>{/each}</select></label>
    <p>{locale.text('standingVrmHint')}</p>
    <p>{locale.text('standingRenderHint')}</p>
    <button type="submit" disabled={!model || !motion}>{locale.text('standingVrmAdd')}</button>
  </fieldset>
</form>
<style>
  fieldset { border:0; padding:0; } label { display:flex; flex-direction:column; gap:5px; margin:12px 0; }
  input,select,textarea { width:100%; box-sizing:border-box; } p { font-size:12px; overflow-wrap:anywhere; }
</style>
