<script lang="ts">
  import { playbackShortcut } from './playback-shortcut';
  import { locale } from '../../shared/i18n/locale.svelte';
  import { onDestroy } from 'svelte';
  import { exclusivePlayback } from '../../shared/exclusive-playback';
  import { startPlayback } from '../../shared/start-playback';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import type { EditorController } from './controller.svelte';
  import PlaybackTransport from './PlaybackTransport.svelte';
  import { PlaybackClock } from './playback-clock';
  import { VideoFrameLoop } from './video-frame-loop';
  import { PlaybackMetrics } from './playback-metrics';
  import PlaybackMeasurement from './PlaybackMeasurement.svelte';
  import { hostApi } from '../../shared/host-api';
  import { RenderStill } from './render-still.svelte';
  let { editor, jobId }: { editor: Pick<EditorController, 'time' | 'seekGeneration' | 'previewVolume'>; jobId: string } = $props();
  const clock = new PlaybackClock();
  const metrics = new PlaybackMetrics();
  const still = (() => new RenderStill(hostApi, jobId))();
  let alive = true, video: HTMLVideoElement, canvas: HTMLCanvasElement;
  let ready = $state(false), error = $state(''), playing = $state(false);
  let duration = $state(0);
  $effect(() => { if (video) video.volume = editor.previewVolume; });
  let animation: number | undefined, settledFrame: number | undefined;
  const frames = new VideoFrameLoop(paint, () => alive && !error && !!video && !video.paused && !video.ended && !video.seeking);
  const frameCallbacks = () => typeof video?.requestVideoFrameCallback === 'function';
  function startFrames() { if (video && !video.paused) { cancelSettledFrame(); frames.start(video); } }
  const url = $derived(/^[0-9a-f-]{36}$/.test(jobId) ? convertFileSrc(jobId, 'videopreview') : '');
  function publish(value: number | null) {
    if (!alive || value === null) return;
    editor.time = value; editor.seekGeneration++;
  }
  function stopClock() {
    if (animation !== undefined) cancelAnimationFrame(animation);
    animation = undefined;
  }
  function fail() {
    error = locale.text('playbackError'); ready = false; playing = false;
    still.dispose(); frames.stop(); cancelSettledFrame(); stopClock(); video?.pause();
  }
  function cancelSettledFrame() {
    if (settledFrame !== undefined) video?.cancelVideoFrameCallback(settledFrame);
    settledFrame = undefined;
  }
  function settleFrame() {
    if (!alive || error || !video?.paused) return;
    paint();
    if (error) return;
    if (video?.paused && !video.seeking) still.load(video.currentTime);
    cancelSettledFrame();
    if (typeof video?.requestVideoFrameCallback === 'function') {
      settledFrame = video.requestVideoFrameCallback(() => { settledFrame = undefined; paint(); });
    }
  }
  function paint() {
    if (!alive || error || !canvas || !video || video.readyState < 2 || video.seeking) return;
    try {
      if (canvas.width !== video.videoWidth) canvas.width = video.videoWidth;
      if (canvas.height !== video.videoHeight) canvas.height = video.videoHeight;
      const context = canvas.getContext('2d');
      if (!context) throw new Error('Canvas unavailable');
      context.drawImage(video, 0, 0);
      metrics.draw(performance.now(), !document.hidden && !video.paused && !video.seeking);
    } catch { fail(); }
  }
  function sample() { if (alive && !error && video) { if (!frameCallbacks()) paint(); if (!error) publish(clock.sample(video.currentTime, video.seeking)); } }
  function schedule() {
    stopClock();
    if (!alive || !video || video.paused || video.ended || error) return;
    animation = requestAnimationFrame(() => { animation = undefined; sample(); schedule(); });
  }
  async function toggle() {
    if (!video.paused) { video.pause(); return; }
    await startPlayback(video, () => { if (alive) fail(); });
  }
  $effect(() => {
    const target = editor.time;
    if (!ready || !video) return;
    const seek = clock.seek(target, video.duration);
    if (seek === null) return;
    if (Math.abs(video.currentTime - seek) < 1e-6) publish(clock.seeked(video.currentTime));
    else { frames.stop(); stopClock(); video.currentTime = seek; }
  });
  onDestroy(() => { alive = false; still.dispose(); frames.stop(); cancelSettledFrame(); stopClock(); video?.pause(); });
</script>
<svelte:window onkeydown={event => playbackShortcut(event, () => { if (ready) void toggle(); })} />
<div class="video-viewport">
  <!-- Draw decoded frames into the page: native video compositing was black at enlarged sizes. -->
  <canvas bind:this={canvas} class:paused={!playing} aria-label={locale.text('previewAccessible')}></canvas>
  {#if !playing && still.image}
    <img class="still" src={`data:image/png;base64,${still.image.png_base64}`} alt={locale.text('previewAccessible')} />
  {/if}
  <!-- Subtitles are composed into the completed movie; transport controls are below. -->
  <!-- svelte-ignore a11y_media_has_caption -->
  <video bind:this={video} use:exclusivePlayback preload="auto" playsinline src={url} aria-hidden="true" tabindex="-1"
    onloadedmetadata={() => { if (!error) { duration = Number.isFinite(video.duration) ? video.duration : 0; ready = duration > 0; } }}
    onloadeddata={settleFrame} oncanplay={settleFrame} onplaying={() => { startFrames(); schedule(); }}
    ontimeupdate={() => { sample(); if (animation === undefined) schedule(); }} onplay={() => { if (error) { video.pause(); return; } still.clear(); playing = true; startFrames(); schedule(); }}
    onpause={() => { playing = false; frames.stop(); stopClock(); sample(); if (alive) still.load(video.currentTime); }}
    onended={() => { playing = false; frames.stop(); stopClock(); sample(); still.load(video.currentTime); }}
    onseeking={() => { still.clear(); frames.stop(); cancelSettledFrame(); stopClock(); }} onseeked={() => { if (!error) { settleFrame(); if (!error) publish(clock.seeked(video.currentTime)); startFrames(); schedule(); } }}
    onerror={fail}></video>
</div>
<PlaybackTransport {playing} {duration} volume={editor.previewVolume} time={editor.time} disabled={!ready || !!error}
  ontoggle={toggle} onseek={(time) => { editor.time = time; }}
  onvolume={(value) => { editor.previewVolume = value; video.volume = value; }} />
{#if error}<p role="alert">{error}</p>{/if}
{#if !error && !playing && still.busy}<p role="status">{locale.text('playbackFrameLoading')}</p>{/if}
{#if !error && !playing && still.error}
  <p role="alert">{locale.text('playbackFrameError')}</p>
  <details>
    <summary>{locale.text('playbackErrorDetails')}</summary>
    <p class="error-details">{still.error}</p>
  </details>
  <button disabled={still.busy || !ready || !!error}
    onclick={() => { if (video?.paused && !video.seeking) still.load(video.currentTime); }}>
    {locale.text('playbackFrameRetry')}
  </button>
{/if}
{#if import.meta.env.VITE_PLAYBACK_MEASUREMENT === '1'}
  <PlaybackMeasurement {metrics} media={() => video} {jobId} editorTime={() => editor.time} />
{/if}
<style>
  .error-details { overflow-wrap:anywhere; white-space:pre-wrap; }
  .video-viewport { position:relative; flex:1 1 80px; min-height:80px; background:#000; }
  canvas { position:absolute; inset:0; display:block; width:100%; height:100%; object-fit:contain; z-index:1; background:#000; }
  canvas.paused { visibility:hidden; }
  .still { position:absolute; inset:0; width:100%; height:100%; object-fit:contain; z-index:2; }
  video { position:absolute; width:1px; height:1px; opacity:0; pointer-events:none; }
</style>
