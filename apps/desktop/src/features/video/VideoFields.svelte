<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { VideoPlayback, VideoProbe } from '@sungeki/api-client';
  let { playback = $bindable(), probe, production = false }: { playback: VideoPlayback; probe: VideoProbe; production?: boolean } = $props();
  const clock = $derived(probe.metadata.video.clock);
  const seconds = $derived(clock.duration_ts === null ? null : clock.duration_ts * clock.time_base.numerator / clock.time_base.denominator);
</script>
<section aria-label={locale.text('videoFieldsTitle')}>
  <h4>{locale.text('videoFieldsTitle')}</h4>
  <p>{locale.text('videoSourceLabel')}{seconds === null ? locale.text('videoDurationUnknown') : `${seconds.toFixed(3)} ${locale.text('videoSeconds')}`} — {locale.text('videoSourceUnchanged')}</p>
  <label>{locale.text('videoSourceStart')}<input type="number" min="0" max="604800" step="0.000001" required
    bind:value={() => playback.source_start_us / 1e6, (value: number) => { playback = { ...playback, source_start_us: Math.round(value * 1e6) }; }} /></label>
  <label>{locale.text('videoSourceDuration')}<input type="number" min="0.000001" max="604800" step="0.000001" required
    bind:value={() => playback.source_duration_us / 1e6, (value: number) => { playback = { ...playback, source_duration_us: Math.round(value * 1e6) }; }} /></label>
  <label class="toggle"><input type="checkbox" bind:checked={playback.looped} />{locale.text('videoSourceLoop')}</label>
  <p>{production ? locale.text('videoProductionDurationHint') : locale.text('videoEditorDurationHint')}</p>
  <label>{locale.text('videoSourceAudio')}<select bind:value={playback.audio_stream}>
    <option value={null}>{locale.text('videoSourceAudioNone')}</option>
    {#each probe.metadata.audio as audio}
      <option value={audio.index} disabled={audio.clock.start_pts === null}>{locale.text('videoSourceAudioStream')} {audio.index}（{audio.channels}ch・{audio.sample_rate}Hz）{audio.clock.start_pts === null ? locale.text('videoAudioStartUnknown') : ''}</option>
    {/each}
  </select></label>
  {#if probe.metadata.audio.length === 0}<p>{locale.text('videoNoAudio')}</p>{/if}
  <label>{locale.text('videoAudioGain')}<input type="number" min="-60" max="24" step="any" required bind:value={playback.gain_db} disabled={playback.audio_stream === null} /></label>
  <p>{locale.text('videoHiddenAudioHint')}</p>
  {#if !production}<p>{locale.text('videoPreviewHint')}</p>{/if}
</section>
<style>
  section { border:1px solid var(--panel-border); border-left:3px solid var(--accent); border-radius:5px; padding:10px; margin-bottom:12px; background:var(--panel-header); }
  h4 { margin:0 0 10px; } p { font-size:11px; line-height:1.5; }
  label { display:flex; flex-direction:column; gap:4px; font-size:12px; margin-bottom:10px; }
  input, select { width:100%; box-sizing:border-box; } .toggle { flex-direction:row; align-items:center; } .toggle input { width:auto; }
</style>
