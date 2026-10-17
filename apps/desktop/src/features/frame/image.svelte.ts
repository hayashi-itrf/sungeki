import { locale } from '../../shared/i18n/locale.svelte';
import type { FramePreviewJob, FramePreviewOutput, FramePreviewBytes, HostApi } from '@sungeki/api-client';

export class FrameImage {
  url = $state('');
  jobId = $state('');
  busy = $state(false);
  error = $state('');
  private generation = 0;
  constructor(private api: Pick<HostApi, 'session'>) {}
  clear() {
    this.generation++;
    if (this.url) URL.revokeObjectURL(this.url);
    this.url = ''; this.jobId = ''; this.busy = false; this.error = '';
  }
  async load(job: FramePreviewJob) {
    if (job.phase !== 'succeeded' || !job.output) { this.clear(); return; }
    await this.loadOutput(job.job_id, job.output, async (offset, length) => {
      const response = await this.api.session({ type: 'frame_preview', request: { type: 'read', args: { job_id: job.job_id, offset, length } } });
      if (response.type !== 'frame_preview_bytes') throw new Error(locale.text('frameImageMismatch'));
      return response.data;
    });
  }
  async loadOutput(id: string, output: FramePreviewOutput, read: (offset: number, length: number) => Promise<FramePreviewBytes>) {
    this.clear();
    const generation = this.generation;
    this.busy = true;
    try {
      if (!Number.isSafeInteger(output.byte_length) || output.byte_length < 1 || output.byte_length > 64 * 1024 * 1024) {
        throw new Error(locale.text('frameImageCapacity'));
      }
      const bytes = new Uint8Array(output.byte_length);
      for (let offset = 0; offset < bytes.length;) {
        const length = Math.min(256 * 1024, bytes.length - offset);
        const data = await read(offset, length);
        if (generation !== this.generation) return;
        if (data.offset !== offset || data.total !== bytes.length) throw new Error(locale.text('frameImageMismatch'));
        const decoded = atob(data.base64);
        if (decoded.length !== length) throw new Error(locale.text('frameImageIncomplete'));
        bytes.set(Uint8Array.from(decoded, c => c.charCodeAt(0)), offset);
        offset += length;
      }
      const digest = await crypto.subtle.digest('SHA-256', bytes);
      if (generation !== this.generation) return;
      const hash = Array.from(new Uint8Array(digest), b => b.toString(16).padStart(2, '0')).join('');
      if (hash !== output.sha256) throw new Error(locale.text('frameImageChanged'));
      this.url = URL.createObjectURL(new Blob([bytes], { type: 'image/png' }));
      this.jobId = id;
    } catch (error) {
      if (generation === this.generation) this.error = error && typeof error === 'object' && 'message' in error ? String(error.message) : String(error);
    } finally { if (generation === this.generation) this.busy = false; }
  }
}
