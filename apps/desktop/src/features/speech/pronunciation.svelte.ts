import { locale } from '../../shared/i18n/locale.svelte';
import type { HostApi, SessionResponse } from '@sungeki/api-client';
import type { EditorController } from '../editor/controller.svelte';
type Result = Extract<SessionResponse, { type: 'pronunciation_preview' }>;
export class PronunciationController {
  subtitle = $state('');
  explicit = $state(false);
  spoken = $state('');
  busy = $state(false);
  error = $state('');
  result = $state.raw<Result | null>(null);
  private input = $state.raw<{ subtitle: string; spoken: string | null } | null>(null);
  private generation = 0;
  constructor(private api: Pick<HostApi, 'session'>) {}
  stale(editor: EditorController) {
    return !!this.result && (editor.drafting || this.result.session_id !== editor.snapshot?.session_id
      || this.result.revision !== (editor.incoming?.revision ?? editor.snapshot?.revision)
      || this.input?.subtitle !== this.subtitle || this.input?.spoken !== (this.explicit ? this.spoken : null));
  }
  reset() { this.generation++; this.busy = false; this.result = null; this.input = null; this.error = ''; }
  async preview(editor: EditorController) {
    if (this.busy || editor.busy || editor.pending || editor.drafting || !editor.snapshot) return;
    const { session_id, revision } = editor.snapshot;
    const input = { subtitle: this.subtitle, spoken: this.explicit ? this.spoken : null };
    const generation = ++this.generation;
    this.busy = true; this.error = ''; this.result = null;
    try {
      const result = await this.api.session({ type: 'preview_pronunciation', session_id,
        expected_revision: revision, subtitle_text: input.subtitle, spoken_text: input.spoken });
      if (generation !== this.generation) return;
      if (result.type !== 'pronunciation_preview' || result.session_id !== session_id || result.revision !== revision) {
        throw new Error(locale.text('pronunciationMismatch'));
      }
      this.input = input; this.result = result;
    } catch (error) {
      if (generation === this.generation) this.error = error && typeof error === 'object' && 'message' in error ? String(error.message) : String(error);
    } finally { if (generation === this.generation) this.busy = false; }
  }
}
