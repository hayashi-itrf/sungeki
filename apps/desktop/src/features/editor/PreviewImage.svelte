<script lang="ts">
  import { onDestroy } from 'svelte';
  import { LatestPreview } from './latest-preview';
  import type { SessionRequest } from '@sungeki/api-client';
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { Transform } from '@sungeki/api-client';
  import { hostApi } from '../../shared/host-api';
  import { requestImagePreview } from './image-preview-request';
  import { queueImage } from './image-preview-queue';
  let { sessionId, revision, assetId, transform: t, generation, clipId, timeUs = 0, video = false, proxyJobId }: {
    sessionId: string; revision: number; assetId: string; transform: Transform; generation: number;
    clipId?: string; timeUs?: number; video?: boolean; proxyJobId?: string;
  } = $props();
  let url = $state(''); let error = $state('');
  type Request = Extract<SessionRequest, { type: 'video_preview' | 'psd_preview' | 'image_preview' }>;
  const images = new LatestPreview<Request>(
    (request, cancelled) => new Promise((resolve, reject) => {
      void queueImage(async () => {
        try { resolve(await requestImagePreview(r => hostApi.session(r), request, cancelled)); }
        catch (cause) { reject(cause); }
      });
    }), value => { url = value; error = ''; }, cause => {
      error = cause && typeof cause === 'object' && 'message' in cause ? String(cause.message) : String(cause);
    });
  $effect(() => {
    // Parent clip objects are recreated while evaluating fades/motion. Only a
    // changed source/revision/reload invalidates pixels, never that object identity.
    const identity = JSON.stringify([sessionId, revision, assetId, clipId, generation, video, proxyJobId]);
    if (images.select(identity)) { url = ''; error = ''; }
    const request: Request = video && clipId
      ? { type: 'video_preview', args: { session_id: sessionId, expected_revision: revision, clip_id: clipId, time_us: timeUs, ...(proxyJobId ? { proxy_job_id: proxyJobId } : {}) } }
      : clipId
      ? { type: 'psd_preview', session_id: sessionId, expected_revision: revision, clip_id: clipId, time_us: timeUs }
      : { type: 'image_preview', session_id: sessionId, expected_revision: revision, asset_id: assetId };
    images.request(request, JSON.stringify(request));
  });
  onDestroy(() => images.dispose());
</script>
{#if url}
  <image aria-label={video ? (proxyJobId ? locale.text('previewProxyVideo') : locale.text('previewSourceVideo')) : locale.text('previewAssetImage')} href={url} x={t.x} y={t.y} width={t.width} height={t.height} preserveAspectRatio="none" transform={t.flip_x ? `translate(${2*t.x+t.width} 0) scale(-1 1)` : undefined} />
{:else}
  <g><title>{error || locale.text('previewImageLoadingTitle')}</title>
    <rect x={t.x} y={t.y} width={t.width} height={t.height} fill="none" stroke="#65758b" stroke-dasharray="8 8" />
    <text x={t.x + 8} y={t.y + 24} fill="#65758b" font-size="20">{error ? (video && proxyJobId ? locale.text('previewProxyUnavailable') : locale.text('previewImageUnavailable')) : locale.text('previewImageLoading')}</text>
  </g>
{/if}
