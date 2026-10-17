<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { uiPerformance } from '../../shared/ui-performance.svelte';
  import TimelineWaveform from './TimelineWaveform.svelte';
  import { clipLabel } from './clip-label';
  import { onDestroy } from 'svelte';
  import type { Clip, SessionSnapshot } from '@sungeki/api-client';
  import type { EditorController } from './controller.svelte';
  import { clipBounds, LANE_HEIGHT, timeLabel } from './timeline-layout';
  import { commitTiming } from './timeline-commit';
  import { targets, timing, selectionBounds, neighbourBounds, type GestureMode, type Timing } from './timeline-gesture';
  let { editor, clip, lane, start, span, multiple, waveformGeneration = 0 }: { editor: EditorController; clip: Clip; lane: number; start: number; span: number; multiple: boolean; waveformGeneration?: number } = $props();
  let draft = $state<Timing | null>(null);
  let gesture: { pointerId: number; snapshot: SessionSnapshot; clip: Clip; ids: string[]; previewIds: ReadonlySet<string>; x: number; scale: number; mode: GestureMode; min: number; max: number; targets: number[]; moved: boolean } | null = null;
  let suppressClick = false;
  let finishDragMeasurement: (() => void) | null = null;
  const displayed = $derived(draft ?? (editor.dragPreview?.revision === editor.snapshot?.revision && editor.dragPreview?.ids.has(clip.id)
    ? { start_us: clip.start_us + editor.dragPreview.delta, duration_us: clip.duration_us } : null));
  const bounds = $derived(clipBounds({ ...clip, ...(displayed ?? {}) }, start, span));
  const audioId = $derived(clip.content.type === 'audio' ? clip.content.asset_id
    : clip.content.type === 'speech' ? clip.content.audio?.asset_id : undefined);
  const audioAsset = $derived(audioId ? editor.snapshot?.project.assets.find(a => a.id === audioId) : null);
  const label = $derived(clip.content.type === 'audio'
    ? editor.snapshot?.project.assets.find(a => a.id === (clip.content.type === 'audio' ? clip.content.asset_id : ''))?.name ?? locale.text('clipAudio')
    : clipLabel(clip.content));
  function discard(event?: PointerEvent) {
    if (!gesture || (event && event.pointerId !== gesture.pointerId)) return;
    finishDragMeasurement?.(); finishDragMeasurement = null;
    gesture = null; draft = null; editor.dragPreview = null;
    editor.acceptIncoming();
  }
  onDestroy(discard);
  function down(event: PointerEvent) {
    if (event.button !== 0 || editor.busy || editor.pending || editor.drafting || !editor.snapshot) return;
    suppressClick = false;
    if (multiple || event.ctrlKey || event.metaKey) {
      event.preventDefault(); editor.selectClip(clip.id, true); suppressClick = true; return;
    }
    const button = event.currentTarget as HTMLButtonElement;
    const rect = button.getBoundingClientRect();
    const width = button.parentElement!.getBoundingClientRect().width;
    if (width <= 0) return;
    const edge = Math.min(8, rect.width / 4);
    const ids = editor.selection.includes(clip.id) ? [...editor.selection] : [clip.id];
    const mode = ids.length > 1 ? 'move' : event.clientX - rect.left < edge && clip.start_us >= start * 1e6 ? 'left'
      : rect.right - event.clientX < edge && clip.start_us + clip.duration_us <= (start + span) * 1e6 ? 'right' : 'move';
    const snapshot = editor.snapshot;
    const scene = snapshot.project.scenes.find(s => s.id === clip.scene_id);
    gesture = { pointerId: event.pointerId, snapshot, clip, ids, previewIds: new Set(ids), x: event.clientX, scale: span * 1e6 / width, mode,
      min: scene?.start_us ?? 0, max: scene ? scene.start_us + scene.duration_us : 604800000000,
      targets: targets(snapshot.project, ids), moved: false };
    if (ids.length > 1) Object.assign(gesture, selectionBounds(snapshot.project, ids, clip));
    else {
      const bounds = neighbourBounds(clip, snapshot.project.clips);
      gesture.min = Math.max(gesture.min, bounds.min);
      gesture.max = Math.min(gesture.max, bounds.max);
    }
    editor.focusSelected(clip.id); editor.time = clip.start_us / 1e6; editor.drafting = true;
    event.preventDefault(); button.focus(); button.setPointerCapture(event.pointerId);
  }
  function move(event: PointerEvent) {
    const g = gesture; if (!g || event.pointerId !== g.pointerId) return;
    if (!g.moved && Math.abs(event.clientX - g.x) < 3) return;
    if (!g.moved) finishDragMeasurement = uiPerformance.beginDrag();
    g.moved = true;
    const fps = g.snapshot.project.canvas.fps;
    draft = timing(g.clip, g.mode, (event.clientX - g.x) * g.scale, g.min, g.max, g.targets,
      8 * g.scale, 1e6 * fps.denominator / fps.numerator, !event.shiftKey);
    if (g.ids.length > 1) editor.dragPreview = { ids: g.previewIds, delta: draft.start_us - g.clip.start_us, revision: g.snapshot.revision };
  }
  async function up(event: PointerEvent) {
    const g = gesture; if (!g || event.pointerId !== g.pointerId) return;
    move(event);
    const next = draft; suppressClick = g.moved;
    finishDragMeasurement?.(); finishDragMeasurement = null;
    // Release pointer ownership, but retain the visual draft until the host settles.
    gesture = null;
    await commitTiming(editor, g.snapshot, g.clip, g.moved ? next : null, g.ids, () => {
      draft = null; editor.dragPreview = null;
    });
  }
</script>
<button class:audio={!!audioAsset} class:selected={editor.selection.includes(clip.id)} class:shape={clip.content.type === 'shape'} class:dragging={!!displayed}
  aria-pressed={editor.selection.includes(clip.id)} disabled={editor.busy || !!editor.pending || (editor.drafting && editor.selected !== clip.id)}
  style:top={`${lane * LANE_HEIGHT + 2}px`} style:left={`${bounds.left}%`} style:width={`${bounds.width}%`}
  title={`${label} / ${timeLabel(clip.start_us / 1e6)}: ${locale.text('timelineDragHint')}`}
  onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={discard} onlostpointercapture={discard}
  onkeydown={e => { if (e.key === 'Escape' && gesture) { e.preventDefault(); suppressClick = true; discard(); } }}
  onclick={event => { if (gesture) return; if (suppressClick) { suppressClick = false; return; } editor.selectClip(clip.id, multiple || event.ctrlKey || event.metaKey); editor.time = clip.start_us / 1e6; }}>
  {#if audioAsset && editor.snapshot && !displayed}<TimelineWaveform {clip} asset={audioAsset} {start} {span} sessionId={editor.snapshot.session_id} revision={editor.snapshot.revision} generation={waveformGeneration} />{/if}
  <span class="clip-label">{#if displayed}{timeLabel(displayed.start_us / 1e6)}〜{timeLabel((displayed.start_us + displayed.duration_us) / 1e6)}{:else}{label}{/if}</span>
</button>
<style>
  button { position:absolute; height:29px; overflow:hidden; white-space:nowrap; text-overflow:ellipsis; padding:3px 7px; min-width:2px; border-radius:4px; color:var(--text); background:color-mix(in srgb, var(--accent) 12%, var(--surface)); touch-action:none; cursor:grab; }
  button::before,button::after { content:''; position:absolute; width:4px; top:5px; bottom:5px; border-left:1px solid var(--muted); opacity:.4; }
  button::before { left:3px; } button::after { right:1px; }
  .clip-label { position:relative; display:block; overflow:hidden; text-overflow:ellipsis; }
  .audio .clip-label { font-size:11px; line-height:12px; align-self:start; }
  .audio { padding-top:1px; padding-bottom:15px; }
  .shape { background:color-mix(in srgb, #32a78c 16%, var(--surface)); }
  .selected { outline:2px solid var(--focus); outline-offset:-2px; }
  .dragging { cursor:grabbing; z-index:2; opacity:.8; }
</style>
