import { expect, test, vi } from 'vitest';
import type { BuildRecipe, SessionRequest, SessionResponse } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/recipes/build-dialogue.json';
import { TemplateLibrary } from './template-library.svelte';
const recipe = () => structuredClone(fixture) as BuildRecipe;
const record = () => ({ id: 'preset', revision: 2, preset: { schema_version: 1, name: 'template', description: 'keep', tags: ['tag'], content: { kind: 'template' as const, recipe: recipe() } } });
function api(fn: (r: SessionRequest) => Promise<SessionResponse>) { return { session: vi.fn(fn) }; }

test('loading detaches every recipe field; updating captures input and preserves metadata', async () => {
  const original = record();
  const host = api(async r => {
    if (r.type !== 'presets') throw new Error();
    if (r.request.action === 'get') return { type: 'preset', record: original };
    if (r.request.action !== 'save') throw new Error();
    return { type: 'preset', record: { id: r.request.id, revision: 3, preset: r.request.preset } };
  });
  const library = new TemplateLibrary(host);
  await library.request({ action: 'get', id: 'preset' });
  expect(library.recipe).toEqual(original.preset.content.recipe);
  library.recipe!.scenes[0]!.lines[0]!.subtitle_text = 'new episode';
  expect(original.preset.content.recipe.scenes[0]!.lines[0]!.subtitle_text).not.toBe('new episode');
  const captured = recipe();
  await library.save(captured, 'updated', true);
  captured.scenes[0]!.lines[0]!.subtitle_text = 'later mutation';
  expect(library.recipe).toEqual(recipe());
  expect(library.selected!.preset!.description).toBe('keep');
  expect(library.selected!.preset!.tags).toEqual(['tag']);
  expect(host.session.mock.calls[1]![0]).toMatchObject({ type: 'presets', request: { action: 'save', id: 'preset', expected_revision: 2 } });
});

test('unknown save outcome retries identical snapshot and prevents competing actions', async () => {
  let fail = true;
  const host = api(async r => {
    if (fail) { fail = false; throw { code: 'IO_ERROR', message: 'lost reply' }; }
    if (r.type !== 'presets' || r.request.action !== 'save') throw new Error();
    return { type: 'preset', record: { id: r.request.id, revision: 1, preset: r.request.preset } };
  });
  const library = new TemplateLibrary(host), input = recipe();
  await library.save(input, 'template');
  const pending = JSON.stringify(library.pending);
  input.title = 'changed after submit';
  await library.request({ action: 'get', id: 'other' });
  expect(host.session).toHaveBeenCalledTimes(1);
  await library.request(library.pending!);
  expect(JSON.stringify(host.session.mock.calls[1]![0])).toBe(JSON.stringify({ type: 'presets', request: JSON.parse(pending) }));
  expect(library.pending).toBeNull();
  expect(library.recipe!.title).toBe(recipe().title);
});

test('mixed library pages keep the continuation cursor even with no templates', async () => {
  const host = api(async () => ({ type: 'presets', items: [{ id: 'voice', revision: 1, name: 'voice', kind: 'voice' }], next_after_id: 'voice' }));
  const library = new TemplateLibrary(host);
  await library.request({ action: 'list', after_id: null, limit: 20 });
  expect(library.items).toEqual([]); expect(library.next).toBe('voice');
});
