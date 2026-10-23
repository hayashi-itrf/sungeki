<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { uiPerformance, percentile } from '../../shared/ui-performance.svelte';
  const response = $derived(percentile(uiPerformance.edits.map(s => s.responseMs), .95));
  const update = $derived(percentile(uiPerformance.edits.map(s => s.updateMs), .95));
  const interval = $derived(percentile(uiPerformance.dragIntervals, .95));
  const show = (value: number | null) => value === null ? locale.text('perfUnmeasured') : `${value.toFixed(1)} ms`;
</script>
<section>
  <h2>{locale.text('perfTitle')}</h2>
  <p>{locale.text('perfHint')}</p>
  <button onclick={() => uiPerformance.setEnabled(!uiPerformance.enabled)}>{uiPerformance.enabled ? locale.text('perfStop') : locale.text('perfStart')}</button>
  <button onclick={() => uiPerformance.clear()}>{locale.text('perfClear')}</button>
  <p role="status">{uiPerformance.enabled ? locale.text(uiPerformance.hidden ? 'perfWaitingVisible' : 'perfActive') : locale.text('perfInactive')} / {locale.text('perfEdits')}: {uiPerformance.edits.length} / {locale.text('perfDrags')}: {uiPerformance.dragCount}</p>
  {#if uiPerformance.enabled && uiPerformance.hidden}<p role="status">{locale.text('perfCurrentlyHidden')}</p>{/if}
  <dl>
    <dt>{locale.text('perfResponse')}</dt><dd>{show(response)}</dd>
    <dt>{locale.text('perfUpdate')}</dt><dd>{show(update)}</dd>
    <dt>{locale.text('perfDrag')}</dt><dd>{show(interval)}</dd>
  </dl>
  <p>{locale.text('perfExcluded')}: {locale.text('perfHidden')} {uiPerformance.exclusions.hidden} / {locale.text('perfInterrupted')} {uiPerformance.exclusions.interrupted} / {locale.text('perfFailed')} {uiPerformance.exclusions.failed} / {locale.text('perfShort')} {uiPerformance.exclusions.short} / {locale.text('perfNoFrames')} {uiPerformance.exclusions.noFrames}</p>
  <p>{locale.text('perfExcludedHint')}</p>
  <p>{locale.text('perfTimingHint')}</p>
  <p>{locale.text('perfDragHint')}</p>
</section>
<style>dl { display:grid; grid-template-columns:1fr auto; gap:8px; } dd { margin:0; } p { line-height:1.6; } button { margin-right:8px; }</style>
