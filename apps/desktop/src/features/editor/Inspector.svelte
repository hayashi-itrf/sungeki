<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { placementCommand } from './placement-command';
  import VrmPoseEditor from '../standing/VrmPoseEditor.svelte';
  import VrmFields from '../standing/VrmFields.svelte';
  import SpriteFields from '../standing/SpriteFields.svelte';
  import VisualMotion from './VisualMotion.svelte';
  import ReactionMotion from './ReactionMotion.svelte';
  import RefreshCredits from './RefreshCredits.svelte';
  import FitVisualTiming from './FitVisualTiming.svelte';
  import VideoFields from '../video/VideoFields.svelte';
  import AudioFields from '../audio/AudioFields.svelte';
  import GainFields from '../speech/GainFields.svelte';
  import SpeechFields from '../speech/SpeechFields.svelte';
  import PsdSpeechFields from '../standing/PsdSpeechFields.svelte';
  import PsdPoseFields from '../standing/PsdPoseFields.svelte';
  import ColorInput from '../../shared/ColorInput.svelte';
  import DuplicateClip from './DuplicateClip.svelte';
  import AssetDetails from './AssetDetails.svelte';
  import type { Command, SubtitleLayout, PsdSpeechBinding, PsdPose } from '@sungeki/api-client';
  import type { EditorController } from './controller.svelte';
  let { editor, quickEditing = false }: { editor: EditorController; quickEditing?: boolean } = $props();
  // The parent keys this form by confirmed revision and selection. Drafts never mutate Project.
  const clip = (() => editor.snapshot?.project.clips.find(c => c.id === editor.selected))();
  const track = $derived(editor.snapshot?.project.tracks.find(t => t.id === clip?.track_id));
  let audioPlayback = $state(clip?.content.type === 'audio' ? { ...clip.content.playback,
    volume_points: clip.content.playback.volume_points.map(p => ({ ...p })),
    ducking: clip.content.playback.ducking ? { ...clip.content.playback.ducking } : null } : null);
  let videoPlayback = $state(clip?.content.type === 'video' ? { ...clip.content.playback } : null);
  let vrm = $state(clip?.content.type === 'vrm' ? { ...clip.content.standing, expressions: { ...clip.content.standing.expressions } } : null);
  let sprite = $state(clip?.content.type === 'sprite' ? { ...clip.content, sprites: { ...clip.content.sprites, poses: clip.content.sprites.poses.map(p => ({ ...p })) } } : null);
  let trackId = $state(clip?.track_id ?? '');
  let psdSpeech = $state<PsdSpeechBinding | null>(clip?.content.type === 'psd' && clip.content.speech ? { ...clip.content.speech } : null);
  let psdPose = $state<PsdPose | null>(clip?.content.type === 'psd'
    ? { ...clip.content.pose, selections: clip.content.pose.selections.map(selection => ({ ...selection })) } : null);
  let text = $state(clip?.content.type === 'text' ? clip.content.text : '');
  let characterId = $state(clip?.content.type === 'speech' ? clip.content.character_id : '');
  let subtitle = $state(clip?.content.type === 'speech' ? clip.content.subtitle_text : '');
  let spoken = $state(clip?.content.type === 'speech' ? clip.content.spoken_text ?? '' : '');
  let explicit = $state(clip?.content.type === 'speech' && clip.content.spoken_text !== null);
  let gainEnabled = $state(clip?.content.type === 'speech' && clip.content.gain_db != null);
  let gainDb = $state(clip?.content.type === 'speech' ? clip.content.gain_db ?? 0 : 0);
  let textLayout = $state<SubtitleLayout | null>(clip?.content.type === 'text' && clip.content.layout ? { ...clip.content.layout } : null);
  let previousTextLayout = $state<SubtitleLayout | null>(null);
  function toggleTextLayout(enabled: boolean) {
    if (enabled) textLayout = previousTextLayout ? { ...previousTextLayout } : { alignment: 'left', wrap: true };
    else { previousTextLayout = textLayout ? { ...textLayout } : null; textLayout = null; }
  }
  let outlineEnabled = $state(clip?.content.type === 'text' && !!clip.content.outline);
  let outlineWidth = $state(clip?.content.type === 'text' ? clip.content.outline?.width ?? 3 : 3);
  let outlineColor = $state(clip?.content.type === 'text' ? clip.content.outline?.color ?? '#000000' : '#000000');
  let fadeIn = $state((clip?.transform.fade?.in_us ?? 0) / 1e6);
  let fadeOut = $state((clip?.transform.fade?.out_us ?? 0) / 1e6);
  let start = $state((clip?.start_us ?? 0) / 1_000_000);
  let duration = $state((clip?.duration_us ?? 1_000_000) / 1_000_000);
  const asset = (() => {
    const content = clip?.content;
    return content?.type === 'image' || content?.type === 'psd' || content?.type === 'audio' || content?.type === 'video' ? editor.snapshot?.project.assets.find(a => a.id === content.asset_id) : undefined;
  })();
  let assetName = $state(asset?.name ?? '');
  let assetSource = $state(asset?.source ?? '');
  let assetCredit = $state(asset?.credit ?? '');
  async function apply(event: SubmitEvent) {
    event.preventDefault();
    if (!clip || [start, duration, fadeIn, fadeOut].some(v => !Number.isFinite(v))) return;
    const placement = placementCommand(clip, { ...clip.transform, flipX: clip.transform.flip_x ?? false, fadeIn, fadeOut });
    const commands: Command[] = placement ? [placement] : [];
    if (Math.round(start * 1_000_000) !== clip.start_us) {
      commands.push({ type: 'move_clip', clip_id: clip.id, start_us: Math.round(start * 1_000_000) });
    }
    if (Math.round(duration * 1_000_000) !== clip.duration_us) {
      commands.push({ type: 'resize_clip', clip_id: clip.id, duration_us: Math.round(duration * 1_000_000) });
    }
    if (trackId !== clip.track_id) commands.push({ type: 'set_clip_track', clip_id: clip.id, track_id: trackId });
    if (clip.content.type === 'text') {
      if (outlineEnabled && (!Number.isInteger(outlineWidth) || outlineWidth < 1 || outlineWidth > 32)) return;
      const outline = outlineEnabled ? { width: outlineWidth, color: outlineColor } : null;
      if (JSON.stringify(outline) !== JSON.stringify(clip.content.outline ?? null)) commands.push({ type: 'set_text_outline', clip_id: clip.id, outline });
      if (JSON.stringify(textLayout) !== JSON.stringify(clip.content.layout ?? null)) commands.push({ type: 'set_text_layout', clip_id: clip.id, layout: textLayout });
      commands.push({ type: 'set_text', clip_id: clip.id, text });
    }
    if (clip.content.type === 'psd' && psdPose && JSON.stringify(psdPose) !== JSON.stringify(clip.content.pose)) {
      commands.push({ type: 'set_psd', clip_id: clip.id, asset_id: clip.content.asset_id, pose: psdPose });
    }
    if (clip.content.type === 'psd' && JSON.stringify(psdSpeech) !== JSON.stringify(clip.content.speech ?? null)) {
      commands.push({ type: 'set_psd_speech', clip_id: clip.id, speech: psdSpeech });
    }
    if (vrm && clip.content.type === 'vrm') commands.push({ type: 'set_vrm', clip_id: clip.id, standing: vrm });
    if (sprite && clip.content.type === 'sprite') commands.push({ type: 'set_sprite', clip_id: clip.id, sprites: sprite.sprites, pose: sprite.pose, character_id: sprite.character_id });
    if (clip.content.type === 'speech') commands.push({ type: 'set_speech', clip_id: clip.id, character_id: characterId, subtitle_text: subtitle, spoken_text: explicit ? spoken : null });
    if (clip.content.type === 'speech') {
      if (gainEnabled && (!Number.isFinite(gainDb) || gainDb < -60 || gainDb > 24)) return;
      const gain = gainEnabled ? gainDb : null;
      if (gain !== clip.content.gain_db) commands.push({ type: 'set_speech_gain', clip_id: clip.id, gain_db: gain });
    }
    if (clip.content.type === 'audio' && audioPlayback) commands.push({ type: 'set_audio', clip_id: clip.id, asset_id: clip.content.asset_id, playback: { ...audioPlayback } });
    if (clip.content.type === 'video' && videoPlayback) commands.push({ type: 'set_video', clip_id: clip.id, asset_id: clip.content.asset_id, playback: { ...videoPlayback } });
    if (asset && (assetName !== asset.name || assetSource !== (asset.source ?? '') || assetCredit !== asset.credit)) {
      commands.push({ type: 'set_asset_metadata', asset_id: asset.id, name: assetName, source: assetSource || null, credit: assetCredit });
    }
    await editor.apply(commands);
  }
</script>
{#if clip?.content.type === 'vrm'}<VrmPoseEditor {editor} {clip} />{/if}
<form onsubmit={apply} oninput={() => { editor.drafting = true; }}>
  <h3>{locale.text('inspectorTitle')}</h3>
  <p>{locale.current === 'en' ? 'Apply timing, content and related settings together.' : '時刻・本文・関連設定は、まとめて適用します。'}</p>
  {#if clip}
    {#if track && !track.visible && clip.content.type !== 'audio'}
      <p class="visibility-note"><strong>{locale.text('inspectorHiddenTitle')}</strong><br />{locale.text('inspectorHiddenHint').replace('{track}', () => track.name)}</p>
    {/if}
    {#if track?.muted && (clip.content.type === 'speech' || clip.content.type === 'audio' || (clip.content.type === 'video' && clip.content.playback.audio_stream !== null))}
      <p class="visibility-note"><strong>{locale.text('inspectorMutedTitle')}</strong><br />{locale.text('inspectorMutedHint').replace('{track}', () => track.name)}</p>
    {/if}
    {#if clip.content.type === 'image'}<AssetDetails {editor} assetId={clip.content.asset_id} />{/if}
    <fieldset disabled={editor.busy || quickEditing || !!editor.pending}>
      {#if asset}
        <details>
          <summary>{locale.text('inspectorAssetMetadata')}</summary>
          <p>{locale.text('inspectorAssetShared')}</p>
          <label>{locale.text('inspectorAssetName')}<input bind:value={assetName} /></label>
          <label>{locale.text('inspectorAssetSource')}<input bind:value={assetSource} placeholder={locale.text('inspectorAssetSourceHint')} /></label>
          <label>{locale.text('inspectorAssetCredit')}<textarea bind:value={assetCredit} rows="3"></textarea></label>
        </details>
      {/if}
      {#if clip.content.type === 'audio' && audioPlayback && asset?.probe.type === 'audio'}
        <AudioFields durationUs={Math.round(duration * 1e6)} onDraft={() => { editor.drafting = true; }} bind:playback={audioPlayback} sourceDuration={asset.probe.info.duration_us} />
      {:else if clip.content.type === 'video' && videoPlayback && asset?.probe.type === 'video'}
        <VideoFields bind:playback={videoPlayback} probe={asset.probe.info} />
      {:else if clip.content.type === 'psd' && asset?.probe.type === 'psd'}
        {#if psdPose}<PsdPoseFields probe={asset.probe.info} bind:pose={psdPose} />{/if}
        <PsdSpeechFields probe={asset.probe.info} characters={editor.snapshot!.project.characters} bind:speech={psdSpeech} />
      {:else if clip.content.type === 'vrm' && vrm}
        <VrmFields changed={() => { editor.drafting = true; }} bind:value={vrm} assets={editor.snapshot!.project.assets} characters={editor.snapshot!.project.characters} />
      {:else if clip.content.type === 'sprite' && sprite}
        <SpriteFields changed={() => { editor.drafting = true; }} bind:value={sprite} assets={editor.snapshot!.project.assets} characters={editor.snapshot!.project.characters} />
      {:else if clip.content.type === 'speech'}
        <SpeechFields characters={editor.snapshot!.project.characters} bind:characterId bind:subtitle bind:spoken bind:explicit />
        <GainFields bind:enabled={gainEnabled} bind:gainDb />
      {:else if clip.content.type === 'text'}
        <label><input type="checkbox" bind:checked={outlineEnabled} />{locale.text('inspectorOutline')}</label>
        {#if outlineEnabled}
          <label>{locale.text('inspectorOutlineWidth')}<input type="number" min="1" max="32" step="1" required bind:value={outlineWidth} /></label>
          <ColorInput label={locale.text('inspectorOutlineColor')} bind:value={outlineColor} />
        {/if}
        <label>{locale.text('inspectorText')}<textarea bind:value={text} rows="3"></textarea></label>
        <label><input type="checkbox" checked={textLayout !== null} onchange={e => toggleTextLayout(e.currentTarget.checked)} />{locale.text('inspectorLayout')}</label>
        {#if textLayout}
          <label>{locale.text('inspectorAlignment')}<select bind:value={textLayout.alignment}><option value="left">{locale.text('alignLeft')}</option><option value="center">{locale.text('alignCenter')}</option><option value="right">{locale.text('alignRight')}</option></select></label>
          <label><input type="checkbox" bind:checked={textLayout.wrap} />{locale.text('inspectorWrap')}</label>
        {/if}
      {/if}
      <label>{locale.text('inspectorTrack')}<select bind:value={trackId}>{#each editor.snapshot!.project.tracks.filter(t => t.kind === (['speech', 'audio'].includes(clip.content.type) ? 'audio' : 'visual')) as t}<option value={t.id}>{t.name}{t.visible ? '' : locale.text('inspectorHidden')}</option>{/each}</select></label>
      <div class="numbers">
        {#if clip.content.type !== 'audio'}
        <label>{locale.text('inspectorFadeIn')}<input type="number" min="0" max={duration} step="0.001" required bind:value={fadeIn}/></label>
          <label>{locale.text('inspectorFadeOut')}<input type="number" min="0" max={duration} step="0.001" required bind:value={fadeOut}/></label>
        {/if}
        <label>{locale.text('inspectorStart')}<input type="number" min="0" step="0.000001" required bind:value={start} /></label>
        <label>{locale.text('inspectorDuration')}<input type="number" min="0.000001" max="604800" step="0.000001" required bind:value={duration} /></label>
      </div>
      <button type="submit">{locale.text('inspectorApply')}</button>
      {#if editor.drafting && !quickEditing}<button type="button" onclick={() => editor.acceptIncoming()}>{locale.text('inspectorDiscard')}</button>{/if}
      <DuplicateClip {editor} {clip} />
      <button type="button" onclick={() => editor.apply([{ type: 'delete_clip', clip_id: clip.id }])}>{locale.text('inspectorDelete')}</button>
    </fieldset>
  {:else}<p>{locale.text('inspectorSelect')}</p>{/if}
</form>
{#if clip && (clip.content.type === 'image' || clip.content.type === 'psd' || clip.content.type === 'video' || clip.content.type === 'sprite' || clip.content.type === 'vrm')}<VisualMotion {editor} {clip} />{/if}
{#if clip?.content.type === 'text' && clip.scene_id}<RefreshCredits {editor} {clip} />{/if}
{#if clip && !['speech', 'audio'].includes(clip.content.type)}<FitVisualTiming {editor} {clip} />{/if}
<style>
  h3 { min-height:52px; display:flex; align-items:center; margin:-14px -14px 14px; padding:11px 14px; background:var(--panel-header); border-bottom:1px solid var(--panel-border); }
  .visibility-note { margin:0 0 12px; padding:10px; border:1px solid var(--panel-border); border-left:3px solid var(--accent); border-radius:4px; background:var(--panel-header); font-size:12px; }
  fieldset { border: 0; padding: 0; }
  details { border:1px solid var(--panel-border); padding:8px; margin-bottom:12px; border-radius:5px; }
  details p { font-size:11px; }
  label { display: flex; flex-direction: column; margin: 0 0 10px; font-size: 13px; gap: 4px; }
  .numbers { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
  input, textarea { width: 100%; box-sizing: border-box; }
  button { padding: 8px 12px; font-size: 13px; margin: 4px 0; }
</style>

{#if clip?.content.type === 'image'}<ReactionMotion {editor} {clip} />{/if}
