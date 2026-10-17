import { locale } from '../../shared/i18n/locale.svelte';
import { editErrorMessage } from '../../shared/edit-error';
import type { Command, EditPreview, HostApi, SessionRequest, SessionSnapshot } from '@sungeki/api-client';
type Edit = Extract<SessionRequest, { type: 'edit' }>;
export class EditPlan {
  request = $state.raw<Edit | null>(null);
  preview = $state.raw<EditPreview | null>(null);
  busy = $state(false);
  error = $state('');
  private generation = 0;
  constructor(private api: Pick<HostApi, 'session'>) {}
  stale(snapshot: SessionSnapshot | null): boolean {
    return !!this.request && (!snapshot || this.request.session_id !== snapshot.session_id || this.request.edit.expected_revision !== snapshot.revision);
  }
  cancel() { this.generation++; this.request = null; this.preview = null; this.error = ''; this.busy = false; }
  async prepare(snapshot: SessionSnapshot, commands: Command[]) {
    this.cancel();
    // Freeze the exact JSON request: editing form arrays later cannot change the reviewed plan.
    this.request = JSON.parse(JSON.stringify({ type: 'edit', session_id: snapshot.session_id,
      edit: { operation_id: crypto.randomUUID(), expected_revision: snapshot.revision, action: { type: 'apply', commands } },
    })) as Edit;
    await this.page(false);
  }
  async page(more: boolean) {
    const request = this.request;
    if (this.busy || !request || request.edit.action.type !== 'apply') return;
    if (more && this.preview?.next_offset == null) return;
    const generation = this.generation;
    this.busy = true; this.error = '';
    try {
      const response = await this.api.session({ type: 'preview_edit', session_id: request.session_id,
        expected_revision: request.edit.expected_revision, commands: request.edit.action.commands,
        offset: more ? this.preview!.next_offset! : 0, limit: 20,
      });
      if (generation !== this.generation) return;
      if (response.type !== 'edit_preview') throw new Error(locale.text('reviewFetchError'));
      const next = response.preview;
      if (next.source_revision !== request.edit.expected_revision
        || (more && this.preview && (next.before_hash !== this.preview.before_hash
          || next.after_hash !== this.preview.after_hash))) {
        throw new Error(locale.text('reviewChangedError'));
      }
      this.preview = more && this.preview ? { ...next, entries: [...this.preview.entries, ...next.entries] } : next;
    } catch (error) {
      if (generation === this.generation) this.error = editErrorMessage(error);
    } finally { if (generation === this.generation) this.busy = false; }
  }
}
