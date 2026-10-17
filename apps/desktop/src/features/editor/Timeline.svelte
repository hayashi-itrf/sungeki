<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import TimelineClip from './TimelineClip.svelte';
  import TrackEditor from './TrackEditor.svelte';
  import { tick, untrack } from 'svelte';
  import type { EditorController } from './controller.svelte';
  import { layoutTracks, LANE_HEIGHT, timeLabel, visibleCells } from './timeline-layout';
  import { followTime, handleTimelineKey, projectDuration } from './timeline-navigation';
  let { editor }: { editor: EditorController } = $props();
  let waveformGeneration = $state(0);
  let multiple = $state(false);
  let collapsed = $state<string[]>([]);
  let range = $state(0); let offset = $state(0); let top = $state(0); let height = $state(150);
  let viewport: HTMLDivElement;
  let project = $derived(editor.snapshot!.project);
  let contentDuration = $derived(projectDuration(project));
  let duration = $derived(Math.max(5, contentDuration));
  let span = $derived(range ? Math.min(range, duration) : duration);
  let start = $derived(Math.max(0, Math.min(offset, duration - span)));
  let rows = $derived(layoutTracks(project.tracks, project.clips, new Set(collapsed)));
  let totalHeight = $derived(rows.length ? rows.at(-1)!.top + rows.at(-1)!.height : 70);
  let visibleRows = $derived(rows.filter(r => r.top + r.height > top - LANE_HEIGHT && r.top < top + height + LANE_HEIGHT));
  let scenes = $derived(project.scenes.filter(s => s.start_us < (start + span) * 1_000_000 && s.start_us + s.duration_us > start * 1_000_000));
  $effect(() => { if (editor.navigationGeneration) untrack(() => { void locate(true); }); });
  $effect(() => { if (editor.seekGeneration) untrack(() => { offset = followTime(editor.time, start, span, duration); }); });
  function keydown(event: KeyboardEvent) {
    if (!event.composedPath().some(target => target instanceof HTMLElement && target.hasAttribute('data-timeline-keyboard'))) return;
    const generation = editor.seekGeneration;
    handleTimelineKey(event, editor, contentDuration, {
      editingText: event.composedPath().some(target => target instanceof HTMLElement &&
        (target.isContentEditable || target.matches('input, textarea, select, [role="textbox"]'))),
      modalOpen: !!document.querySelector('dialog[open]'),
    });
    // Virtualized clips can disappear after seeking. Keep focus on the stable viewport.
    if (editor.seekGeneration !== generation) viewport.focus({ preventScroll: true });
  }
  async function locate(atPlayhead = false) {
    const clip = project.clips.find(c => c.id === editor.selected);
    const time = atPlayhead ? editor.time : clip ? clip.start_us / 1_000_000 : editor.time;
    offset = Math.max(0, Math.min(time - span / 4, duration - span));
    if (clip?.scene_id) collapsed = collapsed.filter(id => id !== clip.scene_id);
    await tick();
    const row = rows.find(r => r.cells.some(c => c.clip.id === clip?.id));
    const cell = row?.cells.find(c => c.clip.id === clip?.id);
    if (row && cell) viewport.scrollTop = row.top + cell.lane * LANE_HEIGHT;
  }
</script>
<svelte:window onkeydown={keydown} />
<div class="timeline" data-timeline-keyboard>
  <div class="toolbar">
    <strong>{locale.text('timelineTitle')}</strong><button onclick={() => waveformGeneration++} title={locale.text('timelineWaveformHint')}>{locale.text('timelineReloadWaveforms')}</button><small>{locale.text('timelineSourceWaveforms')}</small><label><input type="checkbox" bind:checked={multiple} disabled={editor.drafting || editor.busy || !!editor.pending} />{locale.text('timelineMultiple')}</label><TrackEditor {editor} /><span class="time" style:width={`${timeLabel(duration).length}ch`}>{timeLabel(editor.time)}</span>
    <label>{locale.text('timelineRange')} <select bind:value={range} onchange={() => { offset = editor.time; }}><option value={0}>{locale.text('timelineAll')}</option><option value={10}>{locale.text('timeline10Seconds')}</option><option value={30}>{locale.text('timeline30Seconds')}</option><option value={60}>{locale.text('timeline1Minute')}</option><option value={300}>{locale.text('timeline5Minutes')}</option></select></label>
    <button onclick={() => locate()}>{locale.text('timelineLocate')}</button>
  </div>
  <div class="navigation">
    <button aria-label={locale.text('timelinePrevious')} disabled={start <= 0} onclick={() => { offset = Math.max(0, start - span / 2); }}>◀</button>
    <label>{locale.text('timelineStart')} <input aria-label={locale.text('timelineStart')} type="range" min="0" max={Math.max(0, duration - span)} step="0.01" value={start} disabled={span === duration} oninput={e => { offset = e.currentTarget.valueAsNumber; }} /></label>
    <button aria-label={locale.text('timelineNext')} disabled={start + span >= duration} onclick={() => { offset = Math.min(duration - span, start + span / 2); }}>▶</button>
  </div>
  <div class="ruler"><span>{locale.text('timelinePosition')}</span><div>
    <input aria-label={locale.text('timelinePosition')} type="range" min={start} max={start + span} step="0.01" value={Math.max(start, Math.min(editor.time, start + span))} oninput={e => { editor.time = e.currentTarget.valueAsNumber; }} />
    <div class="ticks">{#each [0,1,2,3,4] as i}<span>{timeLabel(start + span * i / 4)}</span>{/each}</div>
  </div></div>
  {#if project.scenes.length}
    <div class="scene-row"><span>{locale.text('timelineScene')}</span><div class="bands">
      {#each scenes as scene (scene.id)}
        {@const left = Math.max(start, scene.start_us / 1_000_000)}
        {@const end = Math.min(start + span, (scene.start_us + scene.duration_us) / 1_000_000)}
        <button aria-expanded={!collapsed.includes(scene.id)} title={`${scene.name}: ${locale.text('timelineSceneHint')}`}
          style:left={`${100 * (left - start) / span}%`} style:width={`${100 * (end - left) / span}%`}
          onclick={() => { collapsed = collapsed.includes(scene.id) ? collapsed.filter(id => id !== scene.id) : [...collapsed, scene.id]; }}>
          {collapsed.includes(scene.id) ? '▸' : '▾'} {scene.name}
        </button>
      {/each}
    </div></div>
  {/if}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex (Keyboard users must be able to focus and scroll this viewport.) -->
  <div class="viewport" bind:this={viewport} bind:clientHeight={height} onscroll={e => { top = e.currentTarget.scrollTop; }} role="region" aria-label={locale.text('timelineTracks')} tabindex="0">
    <div class="content" style:height={`${totalHeight}px`}>
      {#each visibleRows as row (row.track.id)}
        <div class="track" style:top={`${row.top}px`} style:height={`${row.height}px`}>
          <div class="track-label" title={row.track.name}><strong>{row.track.name}</strong>{#if !row.track.visible || row.track.muted}<small class="hidden-badge">{[!row.track.visible ? locale.text('timelineHidden') : '', row.track.muted ? locale.text('timelineMuted') : ''].filter(Boolean).join('・')}</small>{/if}</div><div class="clips" class:hidden-track={!row.track.visible}>
            {#each visibleCells(row, start, start + span, top, height) as cell (cell.clip.id)}
              <TimelineClip {editor} clip={cell.clip} lane={cell.lane} {start} {span} {multiple} {waveformGeneration} />
            {/each}
            {#if editor.time >= start && editor.time <= start + span}<div class="playhead" style:left={`${100 * (editor.time - start) / span}%`}></div>{/if}
          </div>
        </div>
      {/each}
      {#if !rows.length}<p>{locale.text('timelineEmpty')}</p>{/if}
    </div>
  </div>
  {#if editor.drafting}<small>{locale.text('timelineDraftHint')}</small>{/if}
</div>
<style>
  .timeline { height:100%; min-height:0; display:flex; flex-direction:column; padding:8px 14px; gap:5px; }
  .viewport:focus-visible { outline:2px solid var(--focus); outline-offset:-3px; }
  .toolbar,.navigation { display:flex; align-items:center; gap:10px; }
  .toolbar { flex-wrap:wrap; min-height:38px; margin:-8px -14px 3px; padding:8px 14px; background:var(--panel-header); border-bottom:1px solid var(--panel-border); }
  .time { flex:none; white-space:nowrap; text-align:right; font-family:ui-monospace, monospace; font-variant-numeric:tabular-nums; }
  .toolbar strong { margin-right:auto; } label { display:flex; align-items:center; gap:6px; white-space:nowrap; }
  .navigation label { flex:1; font-size:11px; } .navigation input { flex:1; min-width:0; }
  button,select { padding:3px 7px; font-size:12px; } .ruler,.scene-row,.track { display:grid; grid-template-columns:110px minmax(0,1fr); gap:8px; }
  .ruler { font-size:11px; align-items:center; } .ruler input { width:100%; margin:0; padding:0; } .ticks { display:flex; justify-content:space-between; color:var(--muted); }
  .bands { height:25px; position:relative; } .bands button { position:absolute; height:100%; overflow:hidden; white-space:nowrap; text-overflow:ellipsis; }
  .viewport { flex:1; min-height:40px; overflow:auto; scrollbar-gutter:stable; }
  .content { position:relative; } .track { position:absolute; width:100%; }
  .track-label { display:flex; flex-direction:column; align-items:flex-start; gap:2px; overflow:hidden; padding:3px 0; }
  .track strong { max-width:100%; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; font-size:12px; }
  .hidden-badge { font-size:10px; line-height:1; padding:2px 4px; border:1px solid var(--panel-border); border-radius:3px; }
  .hidden-track { border-style:dashed !important; }
  .clips { position:relative; background:var(--background); border:1px solid var(--border); }
  .playhead { position:absolute; width:1px; height:100%; background:var(--accent); pointer-events:none; }
  small { color:var(--muted); }
</style>
