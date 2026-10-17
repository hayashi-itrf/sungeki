<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { timeLabel } from './timeline-layout';
  let { playing, duration, time, volume, disabled, ontoggle, onseek, onvolume }: {
    playing: boolean; duration: number; time: number; volume: number; disabled: boolean;
    ontoggle: () => void; onseek: (time: number) => void; onvolume: (volume: number) => void;
  } = $props();
  function timestamp(seconds: number) {
    const safe = Number.isFinite(seconds) ? Math.max(0, seconds) : 0;
    return timeLabel(safe);
  }
</script>
<div class="transport">
  <button {disabled} onclick={() => onseek(0)}>{locale.text('playbackStart')}</button>
  <button {disabled} onclick={ontoggle}>{locale.text(playing ? 'playbackPause' : 'playbackPlay')}</button>
  <input class="position" type="range" min="0" max={duration || 1} step="0.01" value={duration > 0 ? Math.min(time, duration) : 0}
    {disabled} aria-label={locale.text('playbackPosition')}
    oninput={(event) => onseek(event.currentTarget.valueAsNumber)} />
  <button {disabled} onclick={() => onseek(duration)}>{locale.text('playbackEnd')}</button>
  <span class="time">{timestamp(time)} / {timestamp(duration)}</span>
  <label>{locale.text('playbackVolume')}
    <input type="range" min="0" max="1" step="0.05" value={volume} {disabled}
      oninput={(event) => onvolume(event.currentTarget.valueAsNumber)} />
  </label>
</div>
<style>
  .transport { display:flex; flex-wrap:wrap; align-items:center; gap:8px; flex-shrink:0; background:var(--background); padding:6px 0; border-top:1px solid var(--border); }
  .position { flex:1; min-width:100px; }
  .time { font-size:12px; font-variant-numeric:tabular-nums; }
  label { display:flex; align-items:center; gap:4px; font-size:12px; }
  label input { width:70px; }
  button { min-width:64px; }
</style>
