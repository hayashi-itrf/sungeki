<script lang="ts">
  import { onDestroy } from 'svelte';
  import { locale } from '../../shared/i18n/locale.svelte';
  import { audioPreviewUrl } from '../../shared/audio-preview';
  import { exclusivePlayback } from '../../shared/exclusive-playback';
  import { startPlayback } from '../../shared/start-playback';
  import PlaybackTransport from '../editor/PlaybackTransport.svelte';
  let { jobId, duration }: { jobId: string; duration: number } = $props();
  let audio: HTMLAudioElement;
  let ready = $state(false), playing = $state(false), failed = $state(false);
  let time = $state(0), volume = $state(1);
  let alive = true;
  function fail() {
    if (!alive) return;
    failed = true; ready = false; playing = false; audio?.pause();
  }
  function sample() {
    if (alive && !failed && Number.isFinite(audio.currentTime)) {
      time = audio.ended ? duration : Math.max(0, Math.min(duration, audio.currentTime));
    }
  }
  async function toggle() {
    if (!ready || failed) return;
    if (!audio.paused) { audio.pause(); return; }
    await startPlayback(audio, fail);
  }
  function seek(value: number) {
    if (!ready || failed || !Number.isFinite(value)) return;
    try { audio.currentTime = Math.max(0, Math.min(duration, value)); }
    catch { fail(); }
  }
  onDestroy(() => { alive = false; audio?.pause(); });
</script>
<!-- The host's verified sample count supplies the total; the media clock supplies position. -->
<!-- svelte-ignore a11y_media_has_caption -->
<audio bind:this={audio} use:exclusivePlayback preload="metadata" src={audioPreviewUrl(jobId)}
  onloadedmetadata={() => { if (!failed) ready = Number.isFinite(duration) && duration > 0; }}
  onplay={() => { if (failed) audio.pause(); else playing = true; }}
  onpause={() => { playing = false; sample(); }} onended={() => { playing = false; sample(); }}
  ontimeupdate={sample} onseeked={sample} onerror={fail}></audio>
<PlaybackTransport {playing} {duration} {time} {volume} disabled={!ready || failed}
  ontoggle={toggle} onseek={seek} onvolume={value => { volume = value; audio.volume = value; }} />
<p role="status">{locale.text(playing ? 'audioPreviewPlaying' : 'audioPreviewStopped')}：{time.toFixed(1)} {locale.text('audioPreviewSeconds')}</p>
{#if failed}<p role="alert">{locale.text('audioPreviewPlayError')}</p>{/if}
