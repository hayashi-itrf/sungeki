<script lang="ts">
  import QuitDialog from './QuitDialog.svelte';
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { locale } from '../../shared/i18n/locale.svelte';
  import AddVrm from '../standing/AddVrm.svelte';
  import AddSprite from '../standing/AddSprite.svelte';
  import SceneFade from './SceneFade.svelte';
  import Foreword from './Foreword.svelte';
  import { untrack } from 'svelte';
  import { hostApi } from '../../shared/host-api';
  import { syncSession } from './sync';
  import { EditorController } from './controller.svelte';
  import EditorNotices from './EditorNotices.svelte';
  import DocumentToolbar from './DocumentToolbar.svelte';
  import { ProxySelection } from '../video/proxy-selection.svelte';
  import ProxyPanel from '../video/ProxyPanel.svelte';
  import ImportVideo from '../video/ImportVideo.svelte';
  import ImportAudio from '../audio/ImportAudio.svelte';
  import ImportImage from './ImportImage.svelte';
  import ReviewPanel from '../review/ReviewPanel.svelte';
  import AddCharacter from '../speech/AddCharacter.svelte';
  import Presets from '../presets/Presets.svelte';
  import SpeechJobs from '../speech/SpeechJobs.svelte';
  import LoudnessPanel from '../audio/LoudnessPanel.svelte';
  import AudioPreview from '../audio/AudioPreview.svelte';
  import MixSettings from '../audio/MixSettings.svelte';
  import FramePreview from '../frame/FramePreview.svelte';
  import RenderJobs from '../export/RenderJobs.svelte';
  import ExportSubtitles from '../export/ExportSubtitles.svelte';
  import AddSpeech from '../speech/AddSpeech.svelte';
  import DialogueTemplate from '../speech/DialogueTemplate.svelte';
  import SubtitleStyle from '../speech/SubtitleStyle.svelte';
  import VoiceTuning from '../speech/VoiceTuning.svelte';
  import LayoutSpeech from '../speech/LayoutSpeech.svelte';
  import ImportScript from '../speech/ImportScript.svelte';
  import PronunciationPanel from '../speech/PronunciationPanel.svelte';
  import CanvasEditor from './CanvasEditor.svelte';
  import PlaybackPreview from './PlaybackPreview.svelte';
  import SelectionInspector from './SelectionInspector.svelte';
  import Inspector from './Inspector.svelte';
  import QuickVisual from './QuickVisual.svelte';
  let quickEditing = $state(false);
  import CheckpointPanel from './CheckpointPanel.svelte';
  import Timeline from './Timeline.svelte';
  import TimelineDivider from './TimelineDivider.svelte';
  let { onopensettings }: { onopensettings: () => void } = $props();
  let timelineShare = $state(40);
  import StructurePanel from '../scenes/StructurePanel.svelte';
  let previewGeneration = $state(0);
  let previewExpanded = $state(false);
  let operations: HTMLDialogElement;
  let panel = $state<'scenes' | 'checkpoints'>('scenes');
  const editor = new EditorController(hostApi);
  const proxySelection = new ProxySelection();
  const sessionId = $derived(editor.snapshot?.session_id);
  const chronologicalScenes = $derived([...(editor.snapshot?.project.scenes ?? [])].sort((a, b) => a.start_us - b.start_us));
  $effect(() => {
    const id = sessionId;
    untrack(() => proxySelection.clear());
    if (id) return untrack(startSync);
  });
  function startSync() {
    return syncSession(hostApi, editor.snapshot!, snapshot => editor.receive(snapshot), state => { editor.syncState = state; },
      1000, reset => { editor.checkpointGeneration++; if (reset) editor.checkpointResetGeneration++; }, () => editor.snapshot);
  }
</script>

<QuitDialog {editor} />
<section aria-label={locale.text('editorWorkspace')} class="workspace" style:--timeline-share={timelineShare} class:preview-expanded={previewExpanded} aria-busy={editor.busy}>
  <DocumentToolbar {editor} {onopensettings} />
  <EditorNotices {editor} />
  {#if editor.snapshot}
    <div class="editing">
      <nav class="library" aria-label={locale.text('projectStructure')}>
        <div class="panel-title">{locale.text('projectStructure')}</div>
        <p class="tools-hint">{locale.text('toolsHint')}</p>
        <details class="tool-group">
          <summary>{locale.text('toolsAdd')}</summary>
        <div class="add-buttons">{#key editor.snapshot.session_id}<Presets {editor} />{/key}{#key editor.snapshot.session_id}<AddCharacter {editor} />{/key}{#key editor.snapshot.session_id}<AddSpeech {editor} />{/key}{#key editor.snapshot.session_id}<ImportScript {editor} />{/key}<ImportImage {editor} /><ImportImage {editor} kind="psd" />{#key editor.snapshot.session_id}<AddSprite {editor} />{/key}{#key editor.snapshot.session_id}<AddVrm {editor} />{/key}<ImportAudio {editor} /><ImportVideo {editor} /><button disabled={editor.busy} onclick={() => editor.add('text')}>{locale.text('addText')}</button><button disabled={editor.busy} onclick={() => editor.add('shape')}>{locale.text('addShape')}</button><button disabled={editor.busy || editor.drafting || !!editor.pending} onclick={() => editor.add('blur')}>{locale.text('addBlur')}</button></div>
        </details>
        <details class="tool-group">
          <summary>{locale.text('toolsVoice')}</summary>
          <div class="tool-actions">
            {#key editor.snapshot.session_id}<PronunciationPanel {editor} /><SpeechJobs {editor} />{/key}
            <VoiceTuning {editor} /><LayoutSpeech {editor} /><SubtitleStyle {editor} />
            {#key editor.snapshot.session_id}<MixSettings {editor} /><AudioPreview {editor} /><LoudnessPanel {editor} />{/key}
          </div>
        </details>
        <details class="tool-group">
          <summary>{locale.text('toolsScenes')}</summary>
          <div class="tool-actions"><DialogueTemplate {editor} /><Foreword {editor} />
<SceneFade {editor} />
        {#each chronologicalScenes as scene (scene.id)}
          <button class="scene-row" disabled={editor.busy || editor.drafting || !!editor.pending} onclick={() => editor.revealScene(scene.id)}>
            <span>{scene.name}</span><small>{scene.start_us / 1_000_000} {locale.text('secondsUnit')}</small>
          </button>
        {:else}<p>{locale.text('sceneHint')}</p>{/each}
        <button class="scene-row" onclick={() => { panel = 'scenes'; operations.showModal(); }}>{locale.text('sceneEditing')}</button>
          </div>
        </details>
        <details class="tool-group">
          <summary>{locale.text('toolsExport')}</summary>
          <div class="tool-actions">
            {#key editor.snapshot.session_id}
              <ReviewPanel {editor} /><FramePreview {editor} /><RenderJobs {editor} /><ExportSubtitles {editor} />
              <ProxyPanel {editor} selection={proxySelection} />
            {/key}
          </div>
        </details>
        <details class="tool-group">
          <summary>{locale.text('toolsHistory')}</summary>
          <button class="scene-row" onclick={() => { panel = 'checkpoints'; operations.showModal(); }}>{locale.text('checkpoints')}</button>
        </details>
      </nav>
      <div class="preview-pane">
        <div class="preview-heading"><strong>{locale.text('preview')}</strong><button aria-pressed={previewExpanded} onclick={() => { previewExpanded = !previewExpanded; }}>{previewExpanded ? locale.text('backToEditor') : locale.text('expandPreview')}</button><CanvasEditor {editor} /><button onclick={() => previewGeneration++}>{locale.text('reloadImages')}</button><small>{editor.snapshot.project.canvas.width} × {editor.snapshot.project.canvas.height}</small></div>
        {#key editor.snapshot.session_id}<PlaybackPreview {editor} proxies={proxySelection.forSession(editor.snapshot.session_id)} generation={previewGeneration} />{/key}
      </div>
      <aside class="inspector">
        {#key `${editor.snapshot.session_id}:${editor.selection.join(',')}`}
          {#if editor.selection.length === 1 && editor.snapshot.project.clips.some(c => c.id === editor.selected && c.content.type !== 'audio')}
            <QuickVisual {editor} bind:editing={quickEditing} />
          {/if}
        {/key}
        {#key `${editor.snapshot.session_id}:${editor.snapshot.revision}:${editor.selection.join(',')}:${editor.draftGeneration}`}
          {#if editor.selection.length > 1}<SelectionInspector {editor} />{:else}<Inspector {editor} {quickEditing} />{/if}
        {/key}
      </aside>
    </div>
    {#if !previewExpanded}<TimelineDivider bind:share={timelineShare} />{/if}
    <div class="timeline-pane">{#key editor.snapshot.session_id}<Timeline {editor} />{/key}</div>
    <footer><span>{editor.snapshot.project.scenes.length} {locale.text('scenes')} · {editor.snapshot.project.clips.length} {locale.text('clips')}</span><span role="status">{editor.syncStatus}</span></footer>
  {:else}
    <div class="empty"><h2>{locale.text('beginEditing')}</h2><p>{locale.text('beginEditingHint')}</p><p>{locale.text('beginEditingAssets')}</p></div>
  {/if}
  <dialog use:containDialogFocus bind:this={operations} aria-label={locale.text('editingOperations')}>
    <div class="dialog-heading"><h2>{panel === 'scenes' ? locale.text('sceneEditing') : locale.text('checkpoints')}</h2><button onclick={() => operations.close()}>{locale.text('close')}</button></div>
    <EditorNotices {editor} />
    {#if editor.snapshot}
      {#key editor.snapshot.session_id}
        <div hidden={panel !== 'scenes'}><StructurePanel {editor} /></div>
        <div hidden={panel !== 'checkpoints'}><CheckpointPanel {editor} /></div>
      {/key}
    {/if}
  </dialog>
</section>
<style>
  .workspace { flex:1; min-height:0; display:flex; flex-direction:column; overflow:hidden; background:var(--workspace); }
  .editing { flex:calc(100 - var(--timeline-share)) 1 0; min-height:150px; display:grid; grid-template-columns:210px minmax(0, 1fr) 270px; gap:12px; margin:12px 12px 0; overflow:hidden; }
  .library, .inspector { padding:14px; min-height:0; overflow:auto; background:var(--surface); }
  .library, .inspector, .preview-pane { border:2px solid var(--panel-border); border-top:3px solid var(--panel-edge); border-radius:6px; box-shadow:var(--panel-shadow); }
  .panel-title { min-height:52px; display:flex; align-items:center; font-size:14px; margin:-14px -14px 14px; padding:11px 14px; background:var(--panel-header); border-bottom:1px solid var(--panel-border); font-weight:700; }
  .tools-hint { margin:0 0 12px; font-size:12px; color:var(--muted); }
  .tool-group { margin:8px 0; border:1px solid var(--panel-border); border-radius:5px; }
  .tool-group > summary { padding:10px 8px; font-size:12px; font-weight:600; cursor:pointer; background:var(--panel-header); border-radius:4px; }
  .tool-group[open] > summary { border-bottom:1px solid var(--border); border-radius:4px 4px 0 0; }
  .tool-actions, .add-buttons { padding:8px; display:flex; flex-direction:column; align-items:flex-start; gap:6px; }
  .tool-group > .scene-row { width:calc(100% - 16px); margin:8px; }
  .preview-pane { overflow:auto; min-width:0; min-height:0; padding:0 12px 12px; display:flex; flex-direction:column; gap:10px; background:var(--background); }
  .preview-heading, .dialog-heading, footer { display:flex; justify-content:space-between; align-items:center; gap:12px; }
  .preview-heading { min-height:52px; flex-wrap:wrap; margin:0 -12px; padding:10px 14px; border-bottom:1px solid var(--panel-border); background:var(--panel-header); }
  .preview-heading small { color:var(--muted); }
  .preview-expanded .editing { grid-template-columns:minmax(0,1fr); }
  .preview-expanded .library, .preview-expanded .inspector, .preview-expanded .timeline-pane { display:none; }
  .timeline-pane { flex:var(--timeline-share) 1 0; min-height:220px; overflow:hidden; margin:0 12px 12px; border:2px solid var(--panel-border); border-top:3px solid var(--panel-edge); border-radius:6px; background:var(--surface); box-shadow:var(--panel-shadow); }
  footer { padding:6px 14px; border-top:1px solid var(--border); font-size:11px; color:var(--muted); background:var(--surface); }
  .scene-row { width:100%; display:flex; justify-content:space-between; text-align:left; gap:8px; margin:6px 0; }
  .scene-row span { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .scene-row small { white-space:nowrap; }
  .empty { margin:auto; padding:32px; max-width:640px; text-align:center; }
  @media (max-width:1000px) { .editing { grid-template-columns:155px minmax(0,1fr) 230px; } }

</style>
