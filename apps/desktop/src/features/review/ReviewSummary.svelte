<script lang="ts">
  import type { Diagnostics } from '@sungeki/api-client';
  import { locale } from '../../shared/i18n/locale.svelte';
  let { diagnostics }: { diagnostics: Diagnostics } = $props();
  const counts = $derived.by(() => {
    const result = { error: 0, warning: 0, info: 0 };
    for (const issue of diagnostics.issues) result[issue.severity]++;
    return result;
  });
</script>
<div class="summary">
  <p>{locale.text('resultCounts').replace('{passed}', String(diagnostics.summary.passed)).replace('{failed}', String(diagnostics.summary.failed)).replace('{notRun}', String(diagnostics.summary.not_run))}</p>
  <p>{locale.text('resultIssueCounts').replace('{errors}', String(counts.error)).replace('{warnings}', String(counts.warning)).replace('{info}', String(counts.info))}</p>
</div>
<style>
  .summary { border:1px solid var(--panel-border); border-left:4px solid var(--accent); padding:10px; background:var(--panel-header); border-radius:4px; }
  p { margin:0; }
  p + p { margin-top:6px; font-weight:600; }
</style>
