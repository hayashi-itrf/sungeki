import type { BuildRecipe, BuildReuseLine, BuildReuseRequest, BuildReuseSource, HostApi } from '@sungeki/api-client';
import { locale } from '../../shared/i18n/locale.svelte';
export function reuseTargets(recipe: BuildRecipe) {
  return recipe.scenes.flatMap(scene => scene.lines.map(line => {
    const character = recipe.roles.find(role => role.role === line.role)?.character;
    return { characterId: character?.id, characterName: character?.name ?? '', text: line.subtitle_text,
      spoken: line.spoken_text ?? line.subtitle_text };
  }));
}
export function matchingAudio(recipe: BuildRecipe, index: number, source: BuildReuseSource): BuildReuseLine[] {
  const target = reuseTargets(recipe)[index];
  return target ? source.lines.filter(line => line.has_saved_audio && line.character_id === target.characterId
    && line.subtitle_text === target.text && (line.spoken_text ?? line.subtitle_text) === target.spoken) : [];
}
export class BuildReuseSelection {
  source = $state<BuildReuseSource | null>(null);
  choices = $state<Record<string, number>>({});
  busy = $state(false);
  error = $state('');
  changed = $state(false);
  private signature = '';
  private generation = 0;
  constructor(private api: Pick<HostApi, 'session'>) {}
  dispose() { this.generation++; }
  sync(recipe: BuildRecipe | null) {
    const next = JSON.stringify(recipe);
    if (next !== this.signature) {
      this.changed = Object.keys(this.choices).length > 0;
      this.choices = {}; this.signature = next;
    }
  }
  async load(id: string) {
    const generation = ++this.generation;
    this.source = null; this.choices = {}; this.changed = false; this.error = ''; this.busy = !!id;
    if (!id) return;
    try {
      const response = await this.api.session({ type: 'build_video', request: { type: 'read_reuse_source', job_id: id } });
      if (generation !== this.generation) return;
      if (response.type !== 'build_reuse_source' || response.source.job_id !== id
        || response.source.lines.length > 200
        || response.source.lines.some((line, index) => line.index !== index))
        throw new Error(locale.text('buildReuseMismatch'));
      this.source = response.source;
    } catch (e) { if (generation === this.generation) this.error = e instanceof Error ? e.message : String(e); }
    finally { if (generation === this.generation) this.busy = false; }
  }
  choose(recipe: BuildRecipe, index: number, value: string) {
    this.sync(recipe);
    const next = { ...this.choices }; delete next[index];
    if (value !== '') {
      const sourceIndex = Number(value);
      if (!this.source || !matchingAudio(recipe, index, this.source).some(line => line.index === sourceIndex))
        throw new Error(locale.text('buildReuseMismatch'));
      next[index] = sourceIndex;
    }
    this.choices = next; this.changed = false;
  }
  request(recipe: BuildRecipe): BuildReuseRequest | undefined {
    if (this.busy) throw new Error(locale.text('buildReuseLoading'));
    if (!Object.keys(this.choices).length) return undefined;
    if (!this.source || this.signature !== JSON.stringify(recipe)) throw new Error(locale.text('buildReuseChanged'));
    for (const [index, sourceIndex] of Object.entries(this.choices)) {
      if (!matchingAudio(recipe, Number(index), this.source).some(line => line.index === sourceIndex))
        throw new Error(locale.text('buildReuseMismatch'));
    }
    return { parent_job_id: this.source.job_id, speech: { ...this.choices } };
  }
}
