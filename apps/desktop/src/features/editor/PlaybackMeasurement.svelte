<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { onDestroy } from 'svelte';
  import PlaybackVisibility from './PlaybackVisibility.svelte';
  import type { PlaybackMetrics } from './playback-metrics';
  import { playbackEvents } from './playback-events';
  let { metrics, media, jobId, editorTime, playerMode = 'editor' }: { metrics: PlaybackMetrics; media: () => HTMLVideoElement; jobId: string; editorTime: () => number; playerMode?: 'editor' | 'isolated' } = $props();
  let trace: ReturnType<typeof playbackEvents> | undefined;
  let muted = $state(false);
  let originalMuted: boolean | undefined;
  let audioAtStart: { muted: boolean; volume: number } | undefined;
  function changeMuted(event: Event) {
    const video = media();
    originalMuted ??= video.muted;
    video.muted = (event.currentTarget as HTMLInputElement).checked;
    muted = video.muted;
  }
  let active = $state(false), report = $state<string[]>([]);
  let page = $state(0);
  let frame: number | undefined;
  let observed: HTMLVideoElement | undefined;
  const eligible = (video: HTMLVideoElement) => !document.hidden && !video.paused && !video.seeking && video.readyState >= 2;
  function observe() {
    const video = observed;
    if (!active || !video || typeof video.requestVideoFrameCallback !== 'function') return;
    frame = video.requestVideoFrameCallback((now, metadata) => {
      frame = undefined;
      metrics.frame(now, metadata.mediaTime, video.currentTime, eligible(video), {
        observedAtMs: performance.now(), expectedDisplayTimeMs: metadata.expectedDisplayTime,
        presentationTimeMs: metadata.presentationTime, presentedFrames: metadata.presentedFrames,
      });
      observe();
    });
  }
  const reset = () => metrics.resetSegment();
  function stop() {
    trace?.stop();
    active = false; metrics.stop();
    if (frame !== undefined) observed?.cancelVideoFrameCallback(frame);
    frame = undefined;
    document.removeEventListener('visibilitychange', reset);
    for (const event of ['pause', 'seeking', 'waiting']) observed?.removeEventListener(event, reset);
  }
  function start() {
    stop(); observed = media(); metrics.start(performance.now()); active = true; report = []; page = 0;
    audioAtStart = { muted: observed.muted, volume: observed.volume };
    trace = playbackEvents(observed, editorTime, () => document.visibilityState);
    document.addEventListener('visibilitychange', reset);
    for (const event of ['pause', 'seeking', 'waiting']) observed.addEventListener(event, reset);
    observe();
  }
  function finish() {
    stop();
    const { events = [], ...eventCounts } = trace?.report() ?? {};
    const summary = { jobId, playerMode, visibility: document.visibilityState,
      videoFrameCallbackSupported: typeof observed?.requestVideoFrameCallback === 'function',
      audioAtStart, audioAtEnd: observed ? { muted: observed.muted, volume: observed.volume } : null,
      eventCounts, ...metrics.report() };
    const { frameTrace, ...aggregate } = summary;
    report = [...Object.entries(aggregate).map(([key, value]) => JSON.stringify({ [key]: value })),
      JSON.stringify({ frameTraceInfo: { startedAtMs: frameTrace.startedAtMs, total: frameTrace.total, scope: frameTrace.scope } }),
      ...frameTrace.first.map(sample => JSON.stringify({ firstFrame: sample })),
      JSON.stringify({ worstFrame: frameTrace.worstAbsoluteOffset }),
      ...frameTrace.recent.map(sample => JSON.stringify({ recentFrame: sample })),
      ...events.map(event => JSON.stringify(event))];
  }
  onDestroy(() => { stop(); if (originalMuted !== undefined) media().muted = originalMuted; });
</script>
<details>
  <summary>{locale.text('playbackMeasureTitle')}</summary>
  <p>{locale.text('playbackMeasureHint')}</p>
  <button onclick={start} disabled={active}>{locale.text('playbackMeasureStart')}</button>
  <button onclick={finish} disabled={!active}>{locale.text('playbackMeasureFinish')}</button>
  <label><input type="checkbox" checked={muted} disabled={active} onchange={changeMuted} />
    {locale.text('playbackMeasureMuted')}</label>
  <PlaybackVisibility />
  {#if report.length}<div class="report" aria-label={locale.text('playbackMeasureResults')}>
    {#each report.slice(page * 32, (page + 1) * 32) as line}<p>{line}</p>{/each}
  </div>
    <button disabled={page === 0} onclick={() => page--}>{locale.text('playbackMeasurePrevious')}</button>
    <span>{page + 1} / {Math.ceil(report.length / 32)}</span>
    <button disabled={(page + 1) * 32 >= report.length} onclick={() => page++}>{locale.text('playbackMeasureNext')}</button>
  {/if}
</details>
<style>details { flex-shrink:0; font-size:12px; } .report { max-height:160px; overflow:auto; overflow-wrap:anywhere; font-family:monospace; } p { margin:4px 0; }</style>
