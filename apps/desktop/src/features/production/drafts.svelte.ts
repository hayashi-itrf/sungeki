import type { BuildFormInput } from './recipe';
import type { ForewordInput } from './foreword';
import { newScene } from './scenes';
export type BuildDraft = BuildFormInput & { foreword: ForewordInput; baseUrl: string; expanded: boolean; markdownSource?: string };
function createDraft(baseUrl: string): BuildDraft {
  const draft = $state<BuildDraft>({ preset: 'dialogue-wide-v1', title: '', scenes: [newScene('本文')], bgm: '',
    people: [{ character: '', standing: '' }, { character: '', standing: '' }],
    foreword: { enabled: false, text: '', seconds: 8 }, baseUrl, expanded: false });
  return draft;
}
// Kept by the toolbar, outside the modal lifecycle. Drafts never edit Project.
export class BuildDrafts {
  private entries = new Map<string, { value: BuildDraft }>();
  constructor(private initialUrl: () => string) {}
  entry(sessionId: string): { value: BuildDraft } {
    let entry = this.entries.get(sessionId);
    if (!entry) { entry = { value: createDraft(this.initialUrl()) }; this.entries.set(sessionId, entry); }
    return entry;
  }
  forSession(sessionId: string): BuildDraft { return this.entry(sessionId).value; }
}
