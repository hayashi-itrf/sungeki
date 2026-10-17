<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { Asset, Clip } from '@sungeki/api-client';
  import { hostApi } from '../../shared/host-api';
  import { materialWaveformRange, materialWaveformPath } from './material-waveform';
  import { queueWaveform, validateWaveform, waveformPath, waveformRange } from './waveform-view';
  let { clip, asset, start, span, sessionId, revision, generation }: {
    clip: Clip; asset: Asset; start: number; span: number; sessionId: string; revision: number; generation: number;
  } = $props();
  let fraction = $state(1);
  let paths = $state<string[]>([]), error = $state(''), loading = $state(false);
  $effect(() => {
    void generation;
    const probe = asset.probe.type === 'audio' ? asset.probe.info : null;
    const material = probe && clip.content.type === 'audio' ? materialWaveformRange(clip, probe, start, span) : null;
    const speech = probe && clip.content.type === 'speech' ? waveformRange(clip, probe, start, span) : null;
    const range = material ?? speech;
    paths = []; error = ''; loading = false;
    if (!probe || !range) return;
    fraction = range.fraction;
    const request = { session_id: sessionId, expected_revision: revision, asset_id: asset.id,
      base_frames: material ? material.base : 256, level: range.level, start_bucket: material ? material.bucket : speech!.first, count: range.count };
    let cancelled = false; loading = true;
    void queueWaveform(async () => {
      if (cancelled) return;
      try {
        const next: string[] = [];
        for (let channel = 0; channel < probe.channels; channel++) {
          if (cancelled) return;
          const args = { ...request, channel };
          const response = await hostApi.session({ type: 'analyze_audio', args });
          if (cancelled) return;
          if (response.type !== 'audio_analyzed') throw new Error(locale.text('waveformUnexpected'));
          validateWaveform(response.analysis, args, probe);
          next.push(material ? materialWaveformPath(response.analysis, material) : waveformPath(response.analysis, speech!));
        }
        paths = next;
      } catch (cause) {
        if (!cancelled) error = cause && typeof cause === 'object' && 'message' in cause ? String(cause.message) : String(cause);
      } finally { if (!cancelled) loading = false; }
    });
    return () => { cancelled = true; };
  });
</script>
{#if paths.length}
  <svg style:width={`${fraction * 100}%`} viewBox={`0 0 1000 ${20 * paths.length}`} preserveAspectRatio="none" aria-hidden="true">
    {#each paths as path, i}<path d={path} transform={`translate(0 ${20 * i})`} />{/each}
  </svg>
{:else if loading || error}
  <small title={error || locale.text('waveformLoadingTitle')}>{error ? locale.text('waveformUnavailable') : locale.text('waveformLoading')}</small>
{/if}
<style>
  svg { position:absolute; left:0; bottom:1px; width:100%; height:14px; pointer-events:none; }
  path { stroke:var(--accent); stroke-width:1; vector-effect:non-scaling-stroke; fill:none; opacity:.75; }
  small { position:absolute; left:7px; bottom:0; font-size:9px; color:var(--muted); }
</style>
