<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { VolumePoint } from '@sungeki/api-client';
  import { nextVolumePoint } from './volume-points';
  let { points = $bindable(), durationUs, onDraft }: {
    points: VolumePoint[]; durationUs: number; onDraft: () => void;
  } = $props();
  let expanded = $state((() => points.length > 0)());
  const next = $derived(nextVolumePoint(points, durationUs));
  function update(index: number, field: 'at_us' | 'level', value: number) {
    points = points.map((p, i) => i === index ? { ...p, [field]: value } : p);
    onDraft();
  }
  function add() {
    if (!next) return;
    points = [...points, next].sort((a, b) => a.at_us - b.at_us);
    onDraft();
  }
</script>
<details bind:open={expanded}>
  <summary>{locale.text('volumePointsTitle').replace('{count}', String(points.length))}</summary>
  <p>{locale.text('volumePointsHint')}</p>
  {#each points as point, i}
    <fieldset>
      <legend>{locale.text('volumePointTitle').replace('{index}', String(i + 1))}</legend>
      <label>{locale.text('volumePointTime')}<input aria-label={locale.text('volumePointTimeAccessible').replace('{index}', String(i + 1))} type="number" required step="0.000001"
        min={(i ? points[i - 1].at_us + 1 : 0) / 1e6}
        max={Math.min(durationUs, i + 1 < points.length ? points[i + 1].at_us - 1 : durationUs) / 1e6}
        bind:value={() => point.at_us / 1e6, (v: number) => update(i, 'at_us', Math.round(v * 1e6))} /></label>
      <label>{locale.text('volumePointLevel')}<input aria-label={locale.text('volumePointLevelAccessible').replace('{index}', String(i + 1))} type="number" required min="0" max="100" step="any"
        bind:value={() => point.level * 100, (v: number) => update(i, 'level', v / 100)} /></label>
      <button type="button" aria-label={locale.text('volumePointRemoveAccessible').replace('{index}', String(i + 1))} onclick={() => { points = points.filter((_, n) => n !== i); onDraft(); }}>{locale.text('volumePointRemove')}</button>
    </fieldset>
  {/each}
  <button type="button" disabled={!next} onclick={add}>{locale.text('volumePointAdd')}</button>
  <p>{locale.text('volumePointsApplyHint')}</p>
</details>
<style>
  details { margin:12px 0; border:1px solid var(--panel-border); border-radius:4px; padding:8px; }
  summary { font-size:12px; font-weight:600; cursor:pointer; }
  p { font-size:11px; line-height:1.5; }
  fieldset { margin:8px 0; padding:8px; border:1px solid var(--panel-border); border-radius:4px; }
  legend, label { font-size:12px; }
  label { display:flex; flex-direction:column; gap:4px; margin-bottom:8px; }
  input { width:100%; box-sizing:border-box; }
</style>
