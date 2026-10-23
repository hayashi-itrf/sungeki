import { extraText } from '../../shared/i18n/extra-ui';
import type { BuildRecipe, HostApi, PresetRecord, PresetRequest, PresetSummary } from '@sungeki/api-client';

/** Library persistence only. Loading a template never changes an editor Project. */
export class TemplateLibrary {
  items = $state<PresetSummary[]>([]);
  next = $state<string | null>(null);
  selected = $state<PresetRecord | null>(null);
  recipe = $state<BuildRecipe | null>(null);
  busy = $state(false);
  error = $state('');
  pending = $state.raw<PresetRequest | null>(null);
  private alive = true;
  constructor(private api: Pick<HostApi, 'session'>) {}
  dispose() { this.alive = false; }
  async request(request: PresetRequest) {
    if (this.busy || !this.alive) return;
    if (this.pending && JSON.stringify(request) !== JSON.stringify(this.pending)) return;
    this.busy = true; this.error = '';
    try {
      const response = await this.api.session({ type: 'presets', request });
      if (!this.alive) return;
      if (response.type === 'presets' && request.action === 'list') {
        const templates = response.items.filter(item => item.kind === 'template');
        this.items = request.after_id ? [...this.items, ...templates.filter(v => !this.items.some(old => old.id === v.id))] : templates;
        this.next = response.next_after_id;
      } else if (response.type === 'preset' && request.action !== 'list') {
        if (response.record.id !== request.id) throw new Error(extraText("プリセットの応答が一致しません / Preset response mismatch"));
        const content = response.record.preset?.content;
        if (content && content.kind !== 'template') throw new Error(extraText("制作テンプレートを選択してください / Select a production template"));
        this.selected = response.record;
        this.recipe = content ? JSON.parse(JSON.stringify(content.recipe)) : null;
        if (response.record.preset) {
          const item: PresetSummary = { id: response.record.id, revision: response.record.revision, name: response.record.preset.name, kind: 'template' };
          this.items = [item, ...this.items.filter(old => old.id !== item.id)];
        } else this.items = this.items.filter(old => old.id !== request.id);
      } else throw new Error(extraText("プリセットの応答が一致しません / Preset response mismatch"));
      this.pending = null;
    } catch (e) {
      if (!this.alive) return;
      this.error = e && typeof e === 'object' && 'message' in e ? String(e.message) : String(e);
      const code = e && typeof e === 'object' && 'code' in e ? e.code : undefined;
      this.pending = (request.action === 'save' || request.action === 'delete') && (!code || code === 'IO_ERROR' || code === 'HOST_UNAVAILABLE') ? request : null;
    } finally { if (this.alive) this.busy = false; }
  }
  async save(recipe: BuildRecipe, name: string, update = false) {
    if (update && !this.selected?.preset) return;
    const previous = update ? this.selected : null;
    const request: PresetRequest = { action: 'save', id: previous?.id ?? crypto.randomUUID(), expected_revision: previous?.revision ?? 0,
      preset: { schema_version: 1, name, description: previous?.preset?.description ?? '', tags: previous?.preset?.tags ?? [], content: { kind: 'template', recipe } } };
    await this.request(JSON.parse(JSON.stringify(request)) as PresetRequest);
  }
}
