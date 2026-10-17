import type { HostApi, MarkdownScene } from '@sungeki/api-client';
import { newScene, type SceneInput } from './scenes';
import { parseDialogue } from './dialogue';

export function markdownAppendSummary(existing: SceneInput[], added: SceneInput[]) {
  const all = [...existing, ...added];
  let speeches = 0;
  for (const [index, scene] of all.entries()) {
    try { speeches += parseDialogue(scene.script).length; }
    catch { return { scenes: all.length, speeches: null, invalidScene: index + 1, allowed: false }; }
  }
  return { scenes: all.length, speeches, invalidScene: null, allowed: all.length <= 200 && speeches <= 200 };
}

// Only adapt parsed DTOs to the existing form notation; parsing belongs to host/editor.
export function markdownScenesToForm(scenes: MarkdownScene[]): SceneInput[] {
  return scenes.map(scene => ({ ...newScene(scene.name), script: scene.lines.map(line => {
    if (!['left', 'right'].includes(line.role) || /[\r\n]/.test(line.subtitle_text)
      || (line.spoken_text !== null && /[\r\n]/.test(line.spoken_text))) throw new Error('Invalid Markdown response');
    return `${line.role === 'left' ? '左' : '右'}：${line.subtitle_text}`
      + (line.spoken_text === null ? '' : `\n読み：${line.spoken_text}`);
  }).join('\n') }));
}
export class MarkdownInput {
  busy = $state(false);
  error = $state('');
  result = $state<{ source: string; narration: boolean; scenes: SceneInput[] } | null>(null);
  private generation = 0;
  constructor(private api: Pick<HostApi, 'session'>) {}
  dispose() { this.generation++; }
  async parse(source: string, narration: boolean) {
    const generation = ++this.generation;
    this.busy = true; this.error = ''; this.result = null;
    try {
      const response = await this.api.session({ type: 'parse_markdown_script', args: {
        source, roles: narration ? ['left'] : ['left', 'right'],
      } });
      if (generation !== this.generation) return;
      if (response.type !== 'markdown_script') throw new Error('Invalid Markdown response');
      this.result = { source, narration, scenes: markdownScenesToForm(response.scenes) };
    } catch (e) {
      if (generation === this.generation) this.error = e && typeof e === 'object' && 'message' in e ? String(e.message) : String(e);
    } finally { if (generation === this.generation) this.busy = false; }
  }
}
