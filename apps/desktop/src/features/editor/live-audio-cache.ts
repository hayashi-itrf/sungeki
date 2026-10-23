import { extraText } from '../../shared/i18n/extra-ui';
import type { HostApi, SessionSnapshot } from '@sungeki/api-client';
import type { LiveCue } from './live-audio-plan';
// Session/revision scoped. Only nearby clips are decoded; no output files are created.
export class LiveAudioCache {
  private buffers = new Map<string, AudioBuffer>();
  private pending = new Map<string, Promise<AudioBuffer>>();
  private chain: Promise<unknown> = Promise.resolve();
  private disposed = false;
  private failures = new Map<string, unknown>();
  retry() { this.failures.clear(); }
  constructor(private api: Pick<HostApi, 'session'>, private snapshot: SessionSnapshot,
    private context: AudioContext) {}
  get(id: string) { return this.buffers.get(id); }
  load(cue: LiveCue): Promise<AudioBuffer> {
    if (this.failures.has(cue.clip.id)) return Promise.reject(this.failures.get(cue.clip.id));
    const existing = this.get(cue.clip.id);
    if (existing) return Promise.resolve(existing);
    const loading = this.pending.get(cue.clip.id);
    if (loading) return loading;
    const task = this.chain.catch(() => {}).then(async () => {
      if (this.disposed) throw new Error(extraText("再生を終了しました"));
      let offset = 0, bytes: Uint8Array | undefined;
      do {
        if (this.disposed) throw new Error(extraText("再生を終了しました"));
        const result = await this.api.session({ type: 'read_editing_audio', session_id: this.snapshot.session_id,
          expected_revision: this.snapshot.revision, clip_id: cue.clip.id, offset, length: 1024 * 1024 });
        if (result.type !== 'editing_audio_bytes' || result.offset !== offset || result.total > 128 * 1024 * 1024)
          throw new Error(extraText("音声の取得結果が不正です"));
        bytes ??= new Uint8Array(result.total);
        const part = Uint8Array.from(atob(result.base64), c => c.charCodeAt(0));
        if (!part.length || result.total !== bytes.length || offset + part.length > bytes.length) throw new Error(extraText("音声の取得範囲が不正です"));
        bytes.set(part, offset); offset += part.length;
      } while (offset < bytes.length);
      const buffer = await this.context.decodeAudioData(bytes.buffer as ArrayBuffer);
      if (this.disposed) throw new Error(extraText("再生を終了しました"));
      const size = (b: AudioBuffer) => b.length * b.numberOfChannels * 4;
      if (size(buffer) > 128 * 1024 * 1024) throw new Error(extraText("この音声は編集再生のメモリー上限を超えています。確認用動画で再生してください"));
      let used = [...this.buffers.values()].reduce((n, b) => n + size(b), 0);
      for (const [id, b] of this.buffers) {
        if (used + size(buffer) <= 128 * 1024 * 1024) break;
        this.buffers.delete(id); used -= size(b);
      }
      this.buffers.set(cue.clip.id, buffer); return buffer;
    });
    this.chain = task;
    this.pending.set(cue.clip.id, task);
    void task.catch(error => { this.failures.set(cue.clip.id, error); }).finally(() => this.pending.delete(cue.clip.id));
    return task;
  }
  dispose() { this.disposed = true; this.buffers.clear(); }
}
