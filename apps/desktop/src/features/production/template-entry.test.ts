import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'svelte/server';
import type { BuildRecipe } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/recipes/build-dialogue.json';
import BuildTemplates from './BuildTemplates.svelte';
import { TemplateLibrary } from './template-library.svelte';
import { BuildJobsController } from './jobs.svelte';
import { locale } from '../../shared/i18n/locale.svelte';

afterEach(() => { locale.current = 'ja'; });
test.each(['ja', 'en'] as const)('loaded template is usable without an editor project in %s', language => {
  locale.current = language;
  const session = vi.fn();
  const library = new TemplateLibrary({ session });
  const recipe = structuredClone(fixture) as unknown as BuildRecipe;
  library.selected = { id: 'template', revision: 1, preset: { schema_version: 1, name: 'Saved dialogue',
    description: '', tags: [], content: { kind: 'template', recipe } } };
  library.recipe = structuredClone(recipe);
  const before = JSON.stringify(library.selected);
  const html = render(BuildTemplates, { props: { library, currentRecipe: null, jobs: new BuildJobsController({ session }),
    disabled: false, onbusy: () => {}, loadOnly: true } }).body;
  expect(html).toContain(language === 'ja' ? '保存テンプレートから制作する' : 'Create from a saved template');
  expect(html).toContain(language === 'ja' ? '保存先を選んで制作開始' : 'Choose destination and start');
  expect(html).toContain(recipe.scenes[0]!.lines[0]!.subtitle_text);
  expect(html).not.toContain(language === 'ja' ? '上の設定を新しく保存' : 'Save the settings above as new');
  expect(html).not.toContain(language === 'ja' ? '上の設定で保存版を更新' : 'Update saved version from settings above');
  expect(JSON.stringify(library.selected)).toBe(before);
  expect(session).not.toHaveBeenCalled();
});
