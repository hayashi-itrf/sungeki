<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { ReviewJob, CheckKind, Project } from '@sungeki/api-client';
  import ReviewFrameResults from './ReviewFrameResults.svelte';
  import IssueTargets from './IssueTargets.svelte';
  import AssetFreshnessView from './AssetFreshnessView.svelte';
  import ReviewSummary from './ReviewSummary.svelte';
  import { checkDescription, issueDescription } from './diagnostic-text';
  let { job, revision, project, disabled = false, onclip, onasset, onscene }: { job: ReviewJob; revision: number; project?: Project; disabled?: boolean; onclip?: (id: string, timeUs?: number) => void; onasset?: (id: string) => void; onscene?: (id: string) => void } = $props();
  const labels: Record<CheckKind, string> = $derived({ structure: locale.text('resultStructure'), asset_integrity: locale.text('resultAssets'), text_layout: locale.text('resultText'), audio: locale.text('resultAudio'), representative_frames: locale.text('resultFrames'), visual_review: locale.text('resultVisual') });
  const status = $derived({ passed: locale.text('resultPassed'), failed: locale.text('resultFailed'), not_run: locale.text('resultNotRun') });
  const stale = $derived(job.stale || job.source_revision !== revision);
</script>
<section aria-label={locale.text('resultRegion')}>
  <h3>{locale.text('resultHeading').replace('{revision}', String(job.source_revision))}</h3>
  {#if stale}<p class="notice" role="status">{locale.text('resultStale')}</p>{/if}
  {#if job.error}<p role="alert">{locale.text('resultErrorPrefix')}{job.error.message}</p>{/if}
  {#if job.report}
    <ReviewSummary diagnostics={job.report.diagnostics} />
    <p>{locale.text('resultScope')}</p>
    {#each job.report.diagnostics.issues as issue}
      <div class="issue"><p><strong>{issue.severity === 'error' ? locale.text('resultError') : issue.severity === 'warning' ? locale.text('resultWarning') : locale.text('resultInfo')}</strong>：{issueDescription(issue)}</p>
        {#if project && onclip && onasset}<IssueTargets {issue} {project} {disabled} {stale} {onclip} {onasset} {onscene} />{/if}
      </div>
    {/each}
    {#if job.report.representative_frames}
      <ReviewFrameResults jobId={job.job_id} frames={job.report.representative_frames} {stale} />
    {/if}
    <AssetFreshnessView freshness={job.asset_freshness} watchable={!!job.report.asset_fingerprint} />
    <ul class="checks">
      {#each job.report.diagnostics.checks as check}
        <li class:unexamined={check.status === 'not_run'}><div><strong>{labels[check.kind]}</strong><span>{check.kind === 'representative_frames' && check.status === 'passed' ? locale.text('resultGenerated') : status[check.status]}</span></div><p>{checkDescription(check.description)}</p></li>
      {/each}
    </ul>
  {:else if job.phase === 'failed'}<p>{locale.text('resultFailure')}</p>
  {:else if job.phase === 'cancelled'}<p>{locale.text('resultCancelled')}</p>
  {:else if job.phase === 'interrupted'}<p>{locale.text('resultInterrupted')}</p>
  {:else}<p>{job.progress?.stage ?? locale.text('resultWaiting')}</p>{/if}
  <details><summary>{locale.text('resultRecord')}</summary><p>{locale.text('resultJob')}{job.job_id}<br />{locale.text('resultHash')}{job.content_hash}</p></details>
</section>
<style>
  section { min-width:0; }
  .notice { border:1px solid var(--panel-border); border-left:4px solid var(--accent); padding:10px; background:var(--panel-header); border-radius:4px; }
  .checks { list-style:none; padding:0; display:grid; gap:8px; }
  li { padding:10px; border:1px solid var(--panel-border); border-radius:5px; background:var(--surface); }
  li div { display:flex; justify-content:space-between; gap:10px; }
  li p { margin:5px 0 0; font-size:12px; }
  .unexamined { background:var(--background); border-style:dashed; }
  details p { overflow-wrap:anywhere; font-size:11px; }
  .issue { padding:10px; border:1px solid var(--panel-border); }
</style>
