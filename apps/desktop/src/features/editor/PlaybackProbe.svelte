<script lang="ts">
  // Development comparison: same verified media URL, no canvas or editor clock writes.
  import { onDestroy } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { exclusivePlayback } from '../../shared/exclusive-playback';
  import { startPlayback } from '../../shared/start-playback';
  import { locale } from '../../shared/i18n/locale.svelte';
  import { PlaybackMetrics } from './playback-metrics';
  import PlaybackMeasurement from './PlaybackMeasurement.svelte';
  import PlaybackTransport from './PlaybackTransport.svelte';
  let { jobId, editorTime }: { jobId: string; editorTime: () => number } = $props();
  let video: HTMLVideoElement;
  let time = $state(0), duration = $state(0), volume = $state(1);
  let playing = $state(false), failed = $state(false);
  const metrics = new PlaybackMetrics();
  const url = $derived(/^[0-9a-f-]{36}$/.test(jobId) ? convertFileSrc(jobId, 'videopreview') : '');
  function fail() { failed = true; video?.pause(); }
  async function toggle() {
    if (!video.paused) video.pause();
    else await startPlayback(video, fail);
  }
  onDestroy(() => video?.pause());
</script>
<p>{locale.text('playbackProbeHint')}</p>
<!-- Subtitles are already composed into this completed video. -->
<!-- svelte-ignore a11y_media_has_caption -->
<video bind:this={video} use:exclusivePlayback src={url} preload="auto" playsinline
  onloadedmetadata={() => { duration = Number.isFinite(video.duration) ? video.duration : 0; }}
  ontimeupdate={() => { time = video.currentTime; }}
  onplay={() => { playing = true; }} onpause={() => { playing = false; }}
  onended={() => { playing = false; }} onerror={fail}></video>
<PlaybackTransport {playing} {duration} {volume} {time} disabled={!duration || failed}
  ontoggle={toggle} onseek={(value) => { video.currentTime = value; }}
  onvolume={(value) => { volume = value; video.volume = value; }} />
{#if failed}<p role="alert">{locale.text('playbackError')}</p>{/if}
<PlaybackMeasurement {metrics} media={() => video} {jobId} {editorTime} playerMode="isolated" />
<style>video { display:block; width:100%; min-height:80px; max-height:240px; background:#000; } p { font-size:12px; margin:4px 0; }</style>
