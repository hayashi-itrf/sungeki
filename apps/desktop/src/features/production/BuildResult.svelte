<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { BuildVideoJob, CheckKind } from '@sungeki/api-client';
  import AssetFreshnessView from '../review/AssetFreshnessView.svelte';
  import ReviewFrameResults from '../review/ReviewFrameResults.svelte';
  import ReviewSummary from '../review/ReviewSummary.svelte';
  import BuildPlayback from './BuildPlayback.svelte';
  let { job }: { job: BuildVideoJob } = $props();
  const labels: Record<CheckKind, string> = $derived({ structure: locale.text('buildStructure'), asset_integrity: locale.text('buildAssets'), text_layout: locale.text('buildTextLayout'), audio: locale.text('buildAudio'), representative_frames: locale.text('buildFramesCheck'), visual_review: locale.text('buildVisualReview') });
  const status = $derived({ passed: locale.text('buildPassed'), failed: locale.text('buildFailed'), not_run: locale.text('buildNotRun') });
</script>
{#if job.output}
  {@const output = job.output}
  <label>{locale.text('buildSavedProject')}<input readonly value={output.project_path} onclick={e => e.currentTarget.select()} /></label>
  <label>{locale.text('buildVideo')}<input readonly value={output.render.path} onclick={e => e.currentTarget.select()} /></label>
  <p>{(output.render.byte_length / 1048576).toFixed(1)} MB ／ {output.render.frame_count} {locale.text('buildFrames')}</p>
  {#key job.job_id}<BuildPlayback {job} />{/key}
  <h4>{locale.text('buildReview')}</h4>
  <p>{locale.text('buildQualityHint')}</p>
  <ReviewSummary diagnostics={output.review.diagnostics} />
  {#each output.review.diagnostics.issues as issue}<p role="status">{issue.message}</p>{/each}
  {#each output.review.diagnostics.checks as check}
    <details><summary>{labels[check.kind]}：{check.kind === 'representative_frames' && check.status === 'passed' ? locale.text('resultGenerated') : status[check.status]}</summary><p>{check.description}</p></details>
  {/each}
  {#if job.review_asset_freshness}<AssetFreshnessView freshness={job.review_asset_freshness} watchable={!!output.review.asset_fingerprint} />{/if}
  {#if output.review.representative_frames}
    <ReviewFrameResults jobId={output.review_job_id} frames={output.review.representative_frames} stale={job.stale} />
  {/if}
  <p>{locale.text('buildFileHint')}</p>
{/if}
<style>
  label { display:flex; flex-direction:column; gap:6px; margin:10px 0; }
  input { width:100%; } details { padding:8px; margin:6px 0; border:1px solid var(--panel-border); border-radius:4px; }
</style>
