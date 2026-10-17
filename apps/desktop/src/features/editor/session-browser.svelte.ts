import type { HostApi, ProjectReadResult } from '@sungeki/api-client';
type Summary = Extract<ProjectReadResult['data'], { view: 'summary' }>;
export interface SessionEntry { id: string; summary: Summary | null; revision: number | null; error: string }
const message = (error: unknown) => error && typeof error === 'object' && 'message' in error ? String(error.message) : String(error);
export class SessionBrowserModel {
  entries = $state<SessionEntry[]>([]);
  ids = $state<string[]>([]);
  busy = $state(false);
  loaded = $state(false);
  error = $state('');
  private generation = 0;
  constructor(private api: Pick<HostApi, 'session'>) {}
  dispose() { this.generation++; }
  async refresh() {
    const generation = ++this.generation;
    this.busy = true; this.error = ''; this.loaded = false; this.entries = []; this.ids = [];
    try {
      const result = await this.api.session({ type: 'list' });
      if (generation !== this.generation) return;
      if (result.type !== 'sessions') throw new Error('作品一覧の応答が不正です。');
      this.ids = result.session_ids;
      await this.page(generation);
      if (generation === this.generation) this.loaded = true;
    } catch (error) { if (generation === this.generation) this.error = message(error); }
    finally { if (generation === this.generation) this.busy = false; }
  }
  async more() {
    if (this.busy || this.entries.length >= this.ids.length) return;
    const generation = this.generation; this.busy = true;
    try { await this.page(generation); }
    finally { if (generation === this.generation) this.busy = false; }
  }
  private async entry(id: string): Promise<SessionEntry> {
    try {
      const result = await this.api.session({ type: 'read_project', session_id: id, expected_revision: null,
        query: { view: 'summary', scene_ids: null, clip_ids: null, offset: 0, limit: 20 } });
      if (result.type !== 'project_read' || result.result.data.view !== 'summary') throw new Error('作品の要約を取得できませんでした。');
      return { id, summary: result.result.data, revision: result.result.revision, error: '' };
    } catch (error) { return { id, summary: null, revision: null, error: message(error) }; }
  }
  private async page(generation: number) {
    const next = this.ids.slice(this.entries.length, this.entries.length + 12);
    // Bound concurrent host reads; never fetch full Project/history into the UI.
    for (let i = 0; i < next.length; i += 4) {
      const rows = await Promise.all(next.slice(i, i + 4).map(id => this.entry(id)));
      if (generation !== this.generation) return;
      this.entries = [...this.entries, ...rows];
    }
  }
}
