<script lang="ts">
  import type { LocalVoiceSettings } from '@sungeki/api-client';
  import { locale } from '../../shared/i18n/locale.svelte';
  let { tuning = $bindable() }: { tuning: LocalVoiceSettings } = $props();
  const text = (ja: string, en: string) => locale.current === 'ja' ? ja : en;
</script>
<label>{text('話速', 'Speed')}<input type="number" min="0.5" max="2" step="0.05" bind:value={tuning.speed_scale} required /></label>
<label>{text('声の高さ', 'Pitch')}<input type="number" min="-0.15" max="0.15" step="0.01" bind:value={tuning.pitch_scale} required /></label>
{#if tuning.engine === 'aivis_speech'}
  <label>{text('感情表現の強さ', 'Emotion strength')}<input type="number" min="0" max="2" step="0.05" bind:value={tuning.emotion_scale} required /></label>
  <label>{text('テンポの緩急', 'Tempo dynamics')}<input type="number" min="0" max="2" step="0.05" bind:value={tuning.tempo_dynamics_scale} required /></label>
{:else}
  <label>{text('抑揚', 'Intonation')}<input type="number" min="0" max="2" step="0.05" bind:value={tuning.intonation_scale} required /></label>
{/if}
<label>{text('合成時の音量', 'Synthesis volume')}<input type="number" min="0" max="2" step="0.05" bind:value={tuning.volume_scale} required /></label>
<label>{text('開始前の無音（秒）', 'Leading silence (seconds)')}<input type="number" min="0" max="1.5" step="0.05" bind:value={tuning.pre_phoneme_length} required /></label>
<label>{text('終了後の無音（秒）', 'Trailing silence (seconds)')}<input type="number" min="0" max="1.5" step="0.05" bind:value={tuning.post_phoneme_length} required /></label>
<style>label { display:flex; flex-direction:column; gap:5px; margin:10px 0; }</style>
