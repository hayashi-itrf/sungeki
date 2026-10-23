import { extraText } from '../../shared/i18n/extra-ui';
import type { ExportSubtitlesArgs, HostApi, SubtitlePublication } from '@sungeki/api-client';

export class SubtitlesController {
  busy = $state(false);
  pending = $state.raw<ExportSubtitlesArgs | null>(null);
  result = $state.raw<SubtitlePublication | null>(null);
  error = $state('');
  private alive = true;
  constructor(private api: Pick<HostApi, 'session'>, readonly sessionId: string) {}
  dispose() { this.alive = false; }
  // Forgetting an uncertain request never removes a file that may already exist.
  dismiss() { if (!this.busy) { this.pending = null; this.error = ''; } }
  async choose(input: Omit<ExportSubtitlesArgs, 'path'>, select: () => Promise<string | null>) {
    if (!this.alive || this.busy || this.pending || input.session_id !== this.sessionId) return;
    const captured = { ...input };
    this.busy = true; this.error = '';
    try {
      const path = await select();
      if (!path || !this.alive) return;
      this.pending = { ...captured, path }; this.result = null;
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
    const args = this.pending;
    if (!args) return;
    const response = await this.api.session({ type: 'export_subtitles', args });
    if (!this.alive) return;
    if (response.type !== 'subtitles_exported') throw new Error(extraText("字幕の保存結果を確認できませんでした。"));
    const p = response.publication;
    if (p.session_id !== args.session_id || p.source_revision !== args.expected_revision
      || p.format !== args.format || p.include_text !== args.include_text || !p.path.startsWith('/')
      || !/^[a-f0-9]{64}$/.test(p.sha256) || !/^[a-f0-9]{64}$/.test(p.content_hash)
      || !Number.isSafeInteger(p.byte_length) || p.byte_length < 0) {
      throw new Error(extraText("字幕の保存結果が要求と一致しません。"));
    }
    this.result = p; this.pending = null;
  }
  private reject(error: unknown) {
    if (!this.alive) return;
    this.error = error && typeof error === 'object' && 'message' in error ? String(error.message) : String(error);
    // Keep the original request: a prior lost response may have already published a file,
    // even if a later retry rejects its now-old revision. Explicit dismissal is available.
  }
}
