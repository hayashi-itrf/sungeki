import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'svelte/server';
import type { BuildRecipe, Project } from '@sungeki/api-client';
import recipeFixture from '../../../../../tests/fixtures/recipes/build-dialogue.json';
import projectFixture from '../../../../../tests/fixtures/scene-evaluation.json';
import { locale } from '../../shared/i18n/locale.svelte';
import { EditorController } from '../editor/controller.svelte';
import { BuildJobsController } from './jobs.svelte';
import { recipeFromForm } from './recipe';
import type { BuildDraft } from './drafts.svelte';
import BuildForm from './BuildForm.svelte';
import { TemplateLibrary } from './template-library.svelte';
afterEach(() => { locale.current = 'ja'; });

test.each(['dialogue-wide-v1', 'narration-portrait-v1'] as const)('form language changes preserve the complete recipe and draft: %s', preset => {
  const source = structuredClone(recipeFixture) as unknown as BuildRecipe;
  const project = structuredClone(projectFixture.project) as Project;
  project.assets = source.assets.map(a => ({ ...a, credit: 'Fixture credit', locator: { type: 'absolute', path: `/fixture/${a.id}.png` } }));
  project.characters = source.roles.map(r => r.character);
  project.clips = source.roles.map((r, i) => {
    if (r.standing.type !== 'image') throw Error('image fixture required');
    return { id: `standing-${i}`, track_id: 'track', scene_id: null, start_us: 0, duration_us: 1000000,
      transform: { x: 0, y: 0, width: 100, height: 100, opacity: 1 }, content: { type: 'image', asset_id: r.standing.asset_id } };
  });
  // Exercise actual option rendering, including default pose and literal replacement symbols.
  const standing = project.clips[0]!;
  const speaker = project.characters[0]!.id;
  const image = project.assets[0]!;
  project.assets.push({ ...image, id: 'model-option', name: '$$モデル' }, { ...image, id: 'motion-option' });
  const sprites = { default_pose: '$$通常', poses: [
    { name: '$$通常', closed_asset_id: image.id, open_asset_id: null },
    { name: '$$笑顔', closed_asset_id: image.id, open_asset_id: null },
  ] };
  project.clips.push(
    { ...standing, id: 'sprite-default', content: { type: 'sprite', character_id: speaker, pose: null, sprites } },
    { ...standing, id: 'sprite-explicit', content: { type: 'sprite', character_id: speaker, pose: '$$笑顔', sprites } },
    { ...standing, id: 'vrm-option', content: { type: 'vrm', standing: {
      model_asset_id: 'model-option', motion_asset_id: 'motion-option', source_start_us: 0, expressions: {}, character_id: speaker,
    } } },
  );
  const draft: BuildDraft = { preset, expanded: true, title: '変更しない作品名', baseUrl: 'http://127.0.0.1:50021', bgm: '',
    people: source.roles.map((r, i) => ({ character: r.character.id, standing: `standing-${i}` })),
    foreword: { enabled: true, text: '入力済みの前書き', seconds: 8 }, scenes: [{ name: '入力済みの場面',
      background: source.scenes[0]!.background_asset_id, gapSeconds: 0.15, diagrams: [],
      script: preset === 'dialogue-wide-v1' ? '左：一行目です。\n読み：いちぎょうめです。\n右：続きです。' : '左：一行目です。\n読み：いちぎょうめです。' }] };
  const editor = new EditorController({ session: vi.fn() });
  editor.snapshot = { session_id: 's', revision: 0, dirty: false, document_path: null,
    event_cursor: { host_instance_id: 'h', event_seq: 0 }, undo_operation_id: null, redo_operation_id: null, project };
  const jobs = new BuildJobsController({ session: vi.fn() });
  const templates = new TemplateLibrary({ session: vi.fn() });
  const scene = draft.scenes[0]!;
  scene.fade = { inSeconds: 0.3, outSeconds: 0.4 };
  scene.diagrams = [
    { asset: scene.background, first: 1, last: 1, fade: { inSeconds: 0.1, outSeconds: 0.2 },
      motion: { endX: 123, endY: 456, scale: 75, easing: 'linear' } },
    { asset: scene.background, first: 1, last: 1,
      reaction: { x: 300, y: 200, width: 240, height: 180, scale: 40, fadeIn: 0.1, fadeOut: 0.2 } },
  ];
  const before = JSON.stringify(draft), expected = recipeFromForm(project, draft);
  const projectBefore = JSON.stringify(project);
  locale.current = 'en';
  const english = render(BuildForm, { props: { editor, jobs, draft, templates } }).body;
  for (const label of ['New project name', 'Production format', 'Speaker', 'Background', 'Scene order',
    'Foreword text (up to 4 lines)', 'VOICEVOX address', 'Choose a save location and start production']) expect(english).toContain(label);
  expect(english).toContain('reading:full spoken text'); expect(english).toContain('入力済みの前書き');
  expect(english).not.toContain('新しい作品名');
  for (const option of ['Sprite: $$通常', 'Sprite: $$笑顔', 'VRM: $$モデル']) expect(english).toContain(option);
  expect(english).not.toContain('スプライト：');
  expect(english).toContain(preset === 'dialogue-wide-v1' ? 'new video. Landscape' : 'new video. Portrait');
  for (const label of ['Fade entire scene', 'Move and scale diagram', 'Destination X',
    'Fade illustration', 'Center X', 'Initial scale (%)', 'First dialogue line']) expect(english).toContain(label);
  expect(english).toContain('value="linear" selected');
  expect(english).toContain('value="123"'); expect(english).toContain('value="75"');
  expect(english).not.toContain('説明素材をフェードする');
  expect(recipeFromForm(project, draft)).toEqual(expected);
  locale.current = 'ja';
  const japanese = render(BuildForm, { props: { editor, jobs, draft, templates } }).body;
  expect(japanese).toContain('新しい作品名');
  for (const option of ['スプライト：$$通常', 'スプライト：$$笑顔', 'VRM：$$モデル']) expect(japanese).toContain(option);
  expect(recipeFromForm(project, draft)).toEqual(expected); expect(JSON.stringify(draft)).toBe(before);
  expect(JSON.stringify(project)).toBe(projectBefore);
});
