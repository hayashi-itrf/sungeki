<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { onDestroy } from 'svelte';
  import type { FramePreviewOutput, ReviewFrames, ReviewImageTarget } from '@sungeki/api-client';
  import { hostApi } from '../../shared/host-api';
  import { FrameImage } from '../frame/image.svelte';
  import SamplingCoverage from './SamplingCoverage.svelte';
  let { jobId, frames, stale }: { jobId: string; frames: ReviewFrames; stale: boolean } = $props();
  const image = new FrameImage(hostApi);
  let selectedKey = $state(''), fullSize = $state(false);
  type Item = { label: string; key: string; output: FramePreviewOutput; target: ReviewImageTarget };
  const items = $derived<Item[]>([
    ...frames.contact_sheets.flatMap(page => page.output ? [{ label: locale.text('resultPageLabel').replace('{page}', String(page.page)), key: `page-${page.page}`, output: page.output, target: { type: 'contact_sheet' as const, page: page.page } }] : []),
    ...frames.frames.flatMap(frame => frame.output ? [{ label: locale.text('resultFrameLabel').replace('{seconds}', (frame.sample.time_us / 1_000_000).toFixed(3)), key: `frame-${frame.sample.frame_index}`, output: frame.output, target: { type: 'frame' as const, frame_index: frame.sample.frame_index } }] : []),
  ]);
  const label = $derived(items.find(item => item.key === selectedKey)?.label ?? '');
  async function show(item: Item) {
    selectedKey = item.key;
    const id = jobId, target = item.target;
    await image.loadOutput(`${id}:${item.key}`, item.output, async (offset, length) => {
      const response = await hostApi.session({ type: 'read_review_image', args: { job_id: id, target, offset, length } });
      if (response.type !== 'review_image_bytes') throw new Error(locale.text('reviewImagesMismatch'));
      return response.data;
    });
  }
  onDestroy(() => image.clear());
</script>
<section class="images" aria-label={locale.text('reviewImagesTitle')}>
  <h4>{locale.text('reviewImagesTitle')}</h4>
  {#if stale}<p class="notice">{locale.text('reviewImagesStale')}</p>{/if}
  <div class="choices">{#each items as item (item.key)}
    <button disabled={image.busy} onclick={() => show(item)}>{item.label}</button>
  {:else}<p>{locale.text('reviewImagesEmpty')}</p>{/each}</div>
  {#if image.busy}<p role="status">{locale.text('reviewImagesLoading').replace('{label}', label)}</p><button onclick={() => image.clear()}>{locale.text('reviewImagesStop')}</button>{/if}
  {#if image.error}<p role="alert">{image.error}</p>{/if}
  {#if image.url}
    <p>{label}</p>
    <label><input type="checkbox" bind:checked={fullSize} />{locale.text('reviewImagesSize')}</label>
    <div class="viewport" class:full={fullSize}><img src={image.url} alt={label} onerror={() => { image.clear(); image.error = locale.text('reviewImagesFailure'); }} /></div>
    <button onclick={() => image.clear()}>{locale.text('reviewImagesClose')}</button>
  {/if}
  <p>{locale.text('reviewImagesScope')}</p>
  <p>{locale.text('reviewImagesCoverage').replace('{omitted}', String(frames.sampling.omitted.length)).replace('{unsampled}', String(frames.sampling.unsampled.length))}</p>
  <SamplingCoverage sampling={frames.sampling} />
</section>
<style>
  .images { margin:12px 0; padding:12px; border:1px solid var(--panel-border); border-radius:6px; background:var(--surface); }
  .choices { display:flex; flex-wrap:wrap; gap:6px; max-height:160px; overflow:auto; }
  .viewport { overflow:auto; max-height:65vh; margin-top:10px; background:var(--background); border:1px solid var(--panel-border); }
  img { display:block; width:100%; height:auto; } .full img { width:auto; max-width:none; }
  label { display:flex; align-items:center; gap:8px; }
  .notice { border-left:4px solid var(--accent); padding-left:10px; }
</style>
