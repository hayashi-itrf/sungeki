import { locale } from '../../shared/i18n/locale.svelte';
afterEach(() => { locale.current = 'ja'; });
import { afterEach, expect, test } from 'vitest';
import type { BuildRecipe, Project } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/recipes/build-dialogue.json';
import { recipeFromForm, type BuildFormInput } from './recipe';
function setup() {
  const r = structuredClone(fixture) as unknown as BuildRecipe;
  for (const asset of r.assets) { asset.locator = { type: 'absolute', path: `/fixture/${asset.id}.png` }; asset.credit = 'Fixture credit'; }
  const project = { assets: r.assets, characters: r.roles.map(r => r.character),
    clips: r.roles.map((r,i) => ({ id: `standing-${i}`, content: { type: 'image', asset_id: r.standing.type === 'image' ? r.standing.asset_id : (() => { throw new Error('image fixture required'); })() }, transform: { flip_x: i === 1 } })),
    audio_mix: r.audio_mix, pronunciation_dictionary: r.pronunciation_dictionary } as unknown as Project;
  const input: BuildFormInput = { title: '新しい制作', scenes: [{ name: '本文', gapSeconds: 0.15, script: '左：一行目。\r\n\r\n右:二行目。', background: r.scenes[0]!.background_asset_id, diagrams: [] }], bgm: '',
    people: r.roles.map((r,i) => ({ character: r.character.id, standing: `standing-${i}` })) };
  return { project, input };
}
test('form maps dialogue to common recipe and detaches all source values', () => {
  const { project, input } = setup(); const before = JSON.stringify(project);
  const recipe = recipeFromForm(project,input);
  expect(recipe.scenes[0]!.lines.map(l => [l.role,l.subtitle_text])).toEqual([['left','一行目。'],['right','二行目。']]);
  expect(recipe.roles[1]!.standing.flip_x).toBe(true);
  expect(new Set(recipe.assets.map(a => a.id)).size).toBe(recipe.assets.length);
  expect(JSON.stringify(project)).toBe(before);
  recipe.roles[0]!.character.name = '別名'; expect(JSON.stringify(project)).toBe(before);
  expect(recipe.bgm).toBeNull();
});
test('form rejects missing speaker labels and duplicate participants', () => {
  const { project, input } = setup(); input.scenes[0]!.script = '誰のセリフ？';
  expect(() => recipeFromForm(project,input)).toThrow('1行目');
  input.scenes[0]!.script = '左：'; expect(() => recipeFromForm(project,input)).toThrow('1行目');
  input.scenes[0]!.script = '左：本文'; input.people[1]!.character = input.people[0]!.character;
  expect(() => recipeFromForm(project,input)).toThrow('異なる二人');
});
test('form rejects unresolved sources and missing credits before opening save dialog', () => {
  const { project, input } = setup(); project.assets[0]!.credit = '';
  expect(() => recipeFromForm(project,input)).toThrow('原本パスとクレジット');
});
test('diagram-only assets and credits are carried into the immutable recipe', () => {
  const { project, input } = setup();
  const figure = structuredClone(project.assets[0]!); figure.id = 'diagram-only';
  project.assets.push(figure);
  input.scenes[0]!.diagrams = [{ asset: figure.id, first: 1, last: 2 }];
  const recipe = recipeFromForm(project, input);
  expect(recipe.scenes[0]!.diagrams).toEqual([{ asset_id: figure.id, first_line: 0, last_line: 1 }]);
  expect(recipe.assets.map(a => a.id)).toContain(figure.id);
  input.scenes[0]!.diagrams[0]!.first = 2;
  expect(recipe.scenes[0]!.diagrams[0]!.first_line).toBe(0);
  figure.credit = '';
  expect(() => recipeFromForm(project, input)).toThrow('クレジット');
});
test('enabled foreword is detached and disabled foreword is omitted', () => {
  const { project, input } = setup();
  input.foreword = { enabled: true, text: '前書きです。', seconds: 8 };
  const recipe = recipeFromForm(project, input);
  input.foreword.text = '編集後'; input.foreword.enabled = false;
  expect(recipe.foreword).toEqual({ preset_id: 'foreword-wide-v1', text: '前書きです。', duration_us: 8000000 });
  expect(recipeFromForm(project, input).foreword).toBeNull();
});
test('multiple scenes preserve order, backgrounds and local diagram ranges', () => {
  const { project, input } = setup();
  const other = structuredClone(project.assets[0]!); other.id = 'second-background'; project.assets.push(other);
  input.scenes.push({ name: '結論', gapSeconds: 0.15, background: other.id, script: '右：結論です。', diagrams: [{ asset: other.id, first: 1, last: 1 }] });
  input.scenes.reverse();
  const recipe = recipeFromForm(project, input);
  expect(recipe.scenes.map(s => [s.name, s.background_asset_id, s.lines.length])).toEqual([
    ['結論', other.id, 1], ['本文', input.scenes[1]!.background, 2],
  ]);
  expect(recipe.scenes[0]!.diagrams[0]).toEqual({ asset_id: other.id, first_line: 0, last_line: 0 });
  input.scenes[0]!.name = '変更';
  expect(recipe.scenes[0]!.name).toBe('結論');
  expect(recipe.assets.map(a => a.id)).toContain(other.id);
});
test('scene errors identify the scene and aggregate speech limit is enforced', () => {
  const { project, input } = setup();
  const first = input.scenes[0]!;
  input.scenes.push({ ...first, name: '次', script: '誰？' });
  expect(() => recipeFromForm(project, input)).toThrow('場面2：1行目');
  input.scenes[1]!.script = '左：文\n'.repeat(199);
  expect(() => recipeFromForm(project, input)).toThrow('全場面の合計');
  input.scenes[1]!.script = '左：文\n'.repeat(198);
  expect(recipeFromForm(project, input).scenes).toHaveLength(2);
  input.scenes = [];
  expect(() => recipeFromForm(project, input)).toThrow('場面は1〜200');
});
test('PSD standing chooses its original preset even on the opposite side and rejects unknown originals', async () => {
  const { project, input } = setup();
  const { default: preset } = await import('../../../../../presets/characters/metan-sakamoto-2.1-mouth.json');
  project.clips[0]!.content = {
    type: 'psd', asset_id: project.assets[0]!.id,
    pose: { source_sha256: preset.source_sha256, selections: preset.selections },
    speech: { character_id: input.people[0]!.character, ...preset.speech },
  };
  expect(recipeFromForm(project, input).roles[0]!.preset_id).toBe(preset.preset_id);
  project.clips[0]!.content.pose.source_sha256 = 'unknown-original';
  expect(() => recipeFromForm(project, input)).toThrow('対応プリセット');
});
test('reading directives are carried into the recipe without shifting diagram ranges', () => {
  const { project, input } = setup(); const scene = input.scenes[0]!;
  scene.script = '左：Codexです。\n読み：コーデックスです。\n右：確認します。';
  scene.diagrams = [{ asset: scene.background, first: 2, last: 2 }];
  const recipe = recipeFromForm(project, input);
  expect(recipe.scenes[0]!.lines).toHaveLength(2);
  expect(recipe.scenes[0]!.lines[0]).toMatchObject({ subtitle_text: 'Codexです。', spoken_text: 'コーデックスです。' });
  expect(recipe.scenes[0]!.diagrams[0]!.first_line).toBe(1);
});

test('narration uses only the first speaker without changing the preserved dialogue draft', () => {
  const { project, input } = setup();
  input.preset = 'narration-wide-v1';
  input.scenes[0]!.script = '左：Codexの紹介です。\n読み：コーデックスのしょうかいです。';
  const before = JSON.stringify(input);
  const recipe = recipeFromForm(project, input);
  expect(recipe.preset_id).toBe('narration-wide-v1');
  expect(recipe.roles).toHaveLength(1);
  expect(recipe.roles[0]!.character.id).toBe(input.people[0]!.character);
  expect(recipe.scenes[0]!.lines[0]!.spoken_text).toBe('コーデックスのしょうかいです。');
  expect(JSON.stringify(input)).toBe(before);
  input.preset = 'dialogue-wide-v1';
  expect(recipeFromForm(project, input).roles).toHaveLength(2);
  expect(recipe.roles).toHaveLength(1);
});

test('narration rejects right-speaker lines before save selection instead of silently reassigning them', () => {
  const { project, input } = setup(); input.preset = 'narration-wide-v1';
  expect(() => recipeFromForm(project, input)).toThrow('場面1：一人ナレーション');
  input.scenes[0]!.script = '左：ひとりの本文';
  input.people[1] = { character: '', standing: '' };
  expect(recipeFromForm(project, input).roles).toHaveLength(1);
  input.people[0]!.character = '';
  expect(() => recipeFromForm(project, input)).toThrow('話者と立ち絵');
});


test('portrait selects matching foreword and retains drafts across orientation changes', () => {
  const { project, input } = setup();
  input.preset = 'narration-portrait-v1';
  input.scenes[0]!.script = '左：縦動画です。';
  input.foreword = { enabled: true, text: '前書き', seconds: 8 };
  const before = JSON.stringify(input);
  const portrait = recipeFromForm(project, input);
  expect(portrait.preset_id).toBe('narration-portrait-v1');
  expect(portrait.roles).toHaveLength(1);
  expect(portrait.foreword?.preset_id).toBe('foreword-portrait-v1');
  expect(JSON.stringify(input)).toBe(before);
  input.preset = 'narration-wide-v1';
  expect(recipeFromForm(project, input).foreword?.preset_id).toBe('foreword-wide-v1');
  input.preset = 'narration-portrait-v1';
  input.scenes[0]!.script = '右：変更しないでください。';
  expect(() => recipeFromForm(project, input)).toThrow('場面1：一人ナレーション');
});


test('sprite recipe includes unused pose dependencies and binds the selected speaker', () => {
  const { project, input } = setup();
  const asset = structuredClone(project.assets[0]); asset.id = 'extra-pose'; project.assets.push(asset);
  const original = project.clips[0].content;
  if (original.type !== 'image') throw new Error('fixture');
  project.clips[0].content = { type: 'sprite', character_id: project.characters[0].id, pose: null,
    sprites: { default_pose: '通常', poses: [
      { name: '通常', closed_asset_id: original.asset_id, open_asset_id: null },
      { name: '別ポーズ', closed_asset_id: 'extra-pose', open_asset_id: null },
    ] } };
  const recipe = recipeFromForm(project, input);
  expect(recipe.roles[0].preset_id).toBe('sprite-custom-v1');
  expect(recipe.assets.some(a => a.id === 'extra-pose')).toBe(true);
  expect(recipe.roles[0].standing.type).toBe('sprite');
  project.clips[0].content.character_id = project.characters[1].id;
  expect(() => recipeFromForm(project, input)).toThrow('口パク');
});

test('VRM production retains model, motion, timing, expressions and voice binding with detached values', () => {
  const { project, input } = setup();
  for (const id of ['model', 'motion']) project.assets.push({ ...structuredClone(project.assets[0]!), id });
  const standing = { model_asset_id: 'model', motion_asset_id: 'motion', source_start_us: 250000, expressions: { happy: 0.7 }, character_id: project.characters[0]!.id };
  project.clips[0]!.content = { type: 'vrm', standing };
  const before = JSON.stringify(project);
  const recipe = recipeFromForm(project, input);
  expect(recipe.roles[0]).toMatchObject({ preset_id: 'vrm-custom-v1', standing: { type: 'vrm', standing, flip_x: false } });
  expect(recipe.assets.map(a => a.id)).toEqual(expect.arrayContaining(['model', 'motion']));
  const value = recipe.roles[0]!.standing;
  if (value.type !== 'vrm') throw new Error('VRM required');
  value.standing.expressions.happy = 0;
  expect(JSON.stringify(project)).toBe(before);
  standing.character_id = project.characters[1]!.id;
  expect(() => recipeFromForm(project, input)).toThrow('口パクを設定したVRM');
  standing.character_id = project.characters[0]!.id;
  project.assets.find(a => a.id === 'motion')!.credit = '';
  expect(() => recipeFromForm(project, input)).toThrow('クレジット');
});

test('BGM point retiming is explicit and keeps source duration and points detached', () => {
  const { project, input } = setup();
  project.clips.push({ id: 'bgm', duration_us: 10000000, content: { type: 'audio', asset_id: project.assets[0]!.id,
    playback: { source_start_us: 0, source_duration_us: 1000000, looped: false, gain_db: -12, fade_in_us: 100000,
      fade_out_us: 100000, ducking: null, volume_points: [{ at_us: 5000000, level: 0.5 }] } } } as Project['clips'][number]);
  input.bgm = 'bgm';
  const original = JSON.stringify(project);
  expect(recipeFromForm(project, input).bgm!.volume_point_reference_duration_us).toBeUndefined();
  input.fitBgmVolumePoints = true;
  const bgm = recipeFromForm(project, input).bgm!;
  expect(bgm.volume_point_reference_duration_us).toBe(10000000);
  expect(bgm.playback.volume_points).toEqual([{ at_us: 5000000, level: 0.5 }]);
  bgm.playback.volume_points[0]!.level = 0;
  expect(JSON.stringify(project)).toBe(original);
});

test('English production validation rejects invalid setup without mutating the source', () => {
  const { project, input } = setup();
  const before = structuredClone({ project, input });
  const japanese = recipeFromForm(project, input);
  locale.current = 'en';
  expect(recipeFromForm(project, input)).toEqual(japanese);
  expect(() => recipeFromForm(project, { ...input, title: ' ' })).toThrow('Enter a project title.');
  expect(() => recipeFromForm(project, { ...input, people: [] })).toThrow('Select the speakers required');
  expect(() => recipeFromForm(project, { ...input, people: [input.people[0]!, input.people[0]!] })).toThrow('Select two different speakers.');
  expect(() => recipeFromForm(project, { ...input, people: [{ character: 'missing', standing: 'missing' }, input.people[1]!] })).toThrow('Select a speaker and a standing character.');
  expect(() => recipeFromForm(project, { ...input, bgm: 'missing' })).toThrow('Select an audio clip');
  expect(() => recipeFromForm(project, { ...input, preset: 'narration-portrait-v1' })).toThrow('Scene 1: use only');
  const missingCredit = structuredClone(project); missingCredit.assets[0]!.credit = '';
  expect(() => recipeFromForm(missingCredit, input)).toThrow('original file paths and credits');
  expect({ project, input }).toEqual(before);
});
test('English foreword validation preserves text and time across languages', () => {
  const { project, input } = setup();
  locale.current = 'en';
  for (const text of ['', ' ', 'a\nb\nc\nd\ne', 'a\0b', 'a'.repeat(4097)]) {
    expect(() => recipeFromForm(project, { ...input, foreword: { enabled: true, text, seconds: 8 } })).toThrow('Enter a non-blank foreword');
  }
  for (const seconds of [undefined, NaN, Infinity, 1, 61]) {
    expect(() => recipeFromForm(project, { ...input, foreword: { enabled: true, text: 'Keep this text', seconds } })).toThrow('Set the foreword duration');
  }
  const foreword = { enabled: true, text: ' Keep this text \r\nSecond line', seconds: 8 };
  const english = recipeFromForm(project, { ...input, foreword });
  locale.current = 'ja';
  expect(recipeFromForm(project, { ...input, foreword })).toEqual(english);
  expect(english.foreword).toMatchObject({ text: ' Keep this text \nSecond line', duration_us: 8000000 });
});
