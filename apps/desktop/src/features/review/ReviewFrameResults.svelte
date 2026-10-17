<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { ReviewFrames } from '@sungeki/api-client';
  import ReviewImages from './ReviewImages.svelte';
  let { jobId, frames, stale }: { jobId: string; frames: ReviewFrames; stale: boolean } = $props();
</script>
      <p>{locale.text('resultImageCounts').replace('{frames}', String(frames.frames.filter(frame => frame.output).length)).replace('{pages}', String(frames.contact_sheets.filter(page => page.output).length))}</p>
      {#each frames.frames.filter(frame => frame.error) as frame}
        <p role="alert">{locale.text('resultFrameLabel').replace('{seconds}', (frame.sample.time_us / 1_000_000).toFixed(3))}：{frame.error}</p>
      {/each}
      {#each frames.contact_sheets.filter(page => page.error) as page}
        <p role="alert">{locale.text('resultPageLabel').replace('{page}', String(page.page))}：{page.error}</p>
      {/each}
      {#key jobId}<ReviewImages jobId={jobId} {frames} {stale} />{/key}
