import { extraText } from '../../shared/i18n/extra-ui';
import type { SessionRequest, SessionResponse } from '@sungeki/api-client';
type PreviewRequest = Extract<SessionRequest, { type: 'image_preview' | 'psd_preview' | 'video_preview' }>;
type Send = (request: SessionRequest) => Promise<SessionResponse>;
const delay = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));

// Read-only busy responses can be retried. Never retry stale-revision conflicts,
// missing originals or cancelled views, and never display an obsolete response.
export async function requestImagePreview(send: Send, request: PreviewRequest,
  cancelled: () => boolean, wait = delay): Promise<string | null> {
  for (let attempt = 0; attempt < 3; attempt++) {
    if (cancelled()) return null;
    try {
      const result = await send(request);
      if (cancelled()) return null;
      if (result.type !== 'image_preview') throw new Error(extraText("画像を取得できませんでした"));
      return `data:image/png;base64,${result.png_base64}`;
    } catch (cause) {
      if (cancelled()) return null;
      const busy = cause && typeof cause === 'object' && 'code' in cause && cause.code === 'CONFLICT'
        && 'retryable' in cause && cause.retryable === true;
      if (!busy || attempt === 2) throw cause;
      await wait(250 * (attempt + 1));
    }
  }
  return null;
}
