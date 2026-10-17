<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { MessageKey } from '../../shared/i18n/messages';
  import VolumePoints from './VolumePoints.svelte';
  import type { AudioPlayback, Ducking } from '@sungeki/api-client';
  let { playback = $bindable(), sourceDuration, durationUs, onDraft }: { playback: AudioPlayback; sourceDuration: number; durationUs: number; onDraft: () => void } = $props();
  let previousDucking = $state<Ducking | null>(null);
  function toggleDucking(enabled: boolean) {
    if (enabled) {
      playback = { ...playback, ducking: previousDucking ? { ...previousDucking }
        : { attenuation_db: -12, attack_us: 250000, release_us: 500000 } };
    } else {
      previousDucking = playback.ducking ? { ...playback.ducking } : null;
      playback = { ...playback, ducking: null };
    }
    onDraft();
  }
  type TimeField = 'source_start_us' | 'source_duration_us' | 'fade_in_us' | 'fade_out_us';
  const fields: { key: TimeField; label: MessageKey; min: number }[] = [
    { key: 'source_start_us', label: 'audioFieldsStart', min: 0 },
    { key: 'source_duration_us', label: 'audioFieldsDuration', min: 0.000001 },
    { key: 'fade_in_us', label: 'audioFieldsFadeIn', min: 0 },
    { key: 'fade_out_us', label: 'audioFieldsFadeOut', min: 0 },
  ];
</script>
<section aria-label={locale.text('audioFieldsTitle')}>
  <h4>{locale.text('audioFieldsTitle')}</h4>
  <p>{locale.text('audioFieldsSource').replace('{seconds}', (sourceDuration / 1_000_000).toFixed(3))}</p>
  {#each fields as field}
    <label>{locale.text(field.label)}<input type="number" min={field.min} max="604800" step="0.000001" required
      bind:value={() => playback[field.key] / 1_000_000, (value: number) => { playback = { ...playback, [field.key]: Math.round(value * 1_000_000) }; }} /></label>
  {/each}
  <label class="toggle"><input type="checkbox" bind:checked={playback.looped} />{locale.text('audioFieldsLoop')}</label>
  <label>{locale.text('audioFieldsGain')}<input type="number" min="-60" max="24" step="any" required bind:value={playback.gain_db} /></label>
  <p>{locale.text('audioFieldsLengthHint')}</p>
  <VolumePoints bind:points={playback.volume_points} {durationUs} {onDraft} />
  <fieldset>
    <legend>{locale.text('audioFieldsDucking')}</legend>
    <label class="toggle"><input type="checkbox" checked={playback.ducking !== null}
      onchange={(event) => toggleDucking(event.currentTarget.checked)} />{locale.text('audioFieldsDuckEnabled')}</label>
    {#if playback.ducking}
      <label>{locale.text('audioFieldsAttenuation')}<input type="number" min="-60" max="0" step="any" required bind:value={playback.ducking.attenuation_db} /></label>
      <label>{locale.text('audioFieldsAttack')}<input type="number" min="0" max="604800" step="0.000001" required
        bind:value={() => playback.ducking!.attack_us / 1_000_000, (value: number) => { playback.ducking!.attack_us = Math.round(value * 1_000_000); }} /></label>
      <label>{locale.text('audioFieldsRelease')}<input type="number" min="0" max="604800" step="0.000001" required
        bind:value={() => playback.ducking!.release_us / 1_000_000, (value: number) => { playback.ducking!.release_us = Math.round(value * 1_000_000); }} /></label>
      <p>{locale.text('audioFieldsDuckHint')}</p>
    {/if}
  </fieldset>
</section>
<style>
  section { border:1px solid var(--panel-border); border-left:3px solid var(--accent); border-radius:5px; padding:10px; margin-bottom:12px; background:var(--panel-header); }
  h4 { margin:0 0 10px; } p { font-size:11px; line-height:1.5; }
  fieldset { margin:12px 0 0; border:1px solid var(--panel-border); border-radius:4px; padding:10px; }
  legend { font-size:12px; font-weight:600; }
  label { display:flex; flex-direction:column; gap:4px; font-size:12px; margin-bottom:10px; }
  input { width:100%; box-sizing:border-box; } .toggle { flex-direction:row; align-items:center; } .toggle input { width:auto; }
</style>
