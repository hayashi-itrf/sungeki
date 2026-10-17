import { frameText as t } from './messages';
import type { HostApi, PublishFramePreviewArgs, FramePreviewJob, FramePreviewPublication } from '@sungeki/api-client';

export class PublicationController {
  busy = $state(false);
  pending = $state.raw<PublishFramePreviewArgs | null>(null);
  result = $state.raw<FramePreviewPublication | null>(null);
  error = $state('');
  private expected: FramePreviewJob | null = null;
  private alive = true;
  constructor(private api: Pick<HostApi, 'session'>, readonly sessionId: string) {}
  dispose() { this.alive = false; }
  async choose(job: FramePreviewJob, select: () => Promise<string | null>) {
    if (!this.alive || this.busy || this.pending || job.session_id !== this.sessionId
      || job.phase !== 'succeeded' || !job.output) return;
    this.busy = true; this.error = '';
    // Capture the completed job before opening the native save dialog.
    const expected = { ...job, output: { ...job.output } };
    try {
      const path = await select();
      if (!this.alive || !path) return;
      this.expected = expected;
      this.pending = { job_id: expected.job_id, path };
      this.result = null;
      await this.submit();
    } catch (error) { this.reject(error); }
    finally { if (this.alive) this.busy = false; }
  }
  async retry() {
    if (!this.alive || this.busy || !this.pending) return;
    this.busy = true; this.error = '';
    try { await this.submit(); }
    catch (error) { this.reject(error); }
    finally { if (this.alive) this.busy = false; }
  }
  private async submit() {
    const args = this.pending, expected = this.expected;
    if (!args || !expected?.output) return;
    const response = await this.api.session({ type: 'frame_preview', request: { type: 'publish', args } });
    if (!this.alive) return;
    if (response.type !== 'frame_preview_published') throw new Error(t('保存の応答を確認できませんでした。'));
    const p = response.publication, output = expected.output;
    if (p.job_id !== args.job_id || p.session_id !== this.sessionId
      || p.source_revision !== expected.source_revision || p.content_hash !== expected.content_hash
      || p.output.sha256 !== output.sha256 || p.output.byte_length !== output.byte_length
      || p.output.frame_index !== output.frame_index || p.output.time_us !== output.time_us
      || p.output.width !== output.width || p.output.height !== output.height || p.output.font_sha256 !== output.font_sha256
      || p.output.layout_version !== output.layout_version || !p.output.path.startsWith('/')) {
      throw new Error(t('保存の応答が選択した画像と一致しません。'));
    }
    // The host returns a canonical path, which can differ from the dialog path through symlinks.
    this.result = p; this.pending = null; this.expected = null;
  }
  private reject(error: unknown) {
    if (!this.alive) return;
    this.error = error && typeof error === 'object' && 'message' in error ? String(error.message) : String(error);
    const code = error && typeof error === 'object' && 'code' in error ? String(error.code) : '';
    if (['VALIDATION_FAILED', 'OUTPUT_EXISTS', 'EXTERNAL_MODIFICATION', 'NOT_FOUND', 'CANCELLED', 'PROTOCOL_MISMATCH'].includes(code)) {
      this.pending = null; this.expected = null;
    }
    // Transport/I/O errors, durability uncertainty and busy conflicts retain the exact request.
  }
}
