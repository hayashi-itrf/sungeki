import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, symlink, rm, realpath } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { manifest } from './manifest.mjs';
import { buildProfile, writeCandidate, readJson } from './profile.mjs';
async function fixture(t) {
  const root = await realpath(await mkdtemp(path.join(os.tmpdir(), 'vvm-profile-')));
  t.after(() => rm(root, { recursive: true, force: true }));
  for (const dir of ['work', 'engine', 'runtime', 'text', 'models/voice']) await mkdir(path.join(root, dir), { recursive: true });
  for (const file of ['engine/server_fastapi.py', 'engine/config.py', 'engine/config.yml', 'runtime/python', 'text/encoder', 'models/voice/model.safetensors', 'models/voice/config.json', 'models/voice/style_vectors.npy']) await writeFile(path.join(root, file), 'fixture');
  const model = path.join(root, 'models/voice/model.safetensors');
  const catalog = { 0: { model_path: model, config_path: path.join(root, 'models/voice/config.json'), device: 'cpu', spk2id: { Speaker: 0 }, id2spk: { 0: 'Speaker' }, style2id: { Neutral: 0 } } };
  await writeFile(path.join(root, 'catalog.json'), JSON.stringify(catalog));
  return { root, catalog, input: { id: 'local-jp', name: '日本語', endpoint: 'http://127.0.0.1:5050', cpu_only: true, engine_root: path.join(root, 'engine'), work_root: path.join(root, 'work'), python: path.join(root, 'runtime/python'), model, catalog: path.join(root, 'catalog.json'), engine_roots: { runtime: path.join(root, 'runtime') }, text_roots: { jp: path.join(root, 'text') } } };
}
test('candidate records explicit originals without modifying them or installing settings', async t => {
  const { root, input } = await fixture(t);
  const result = await buildProfile(input), profile = result.profiles[0];
  assert.equal(result.format_version, 1);
  assert.equal(profile.binding.originals.model, input.model);
  assert.equal(profile.binding.engine.executable, input.python);
  assert.equal(profile.binding.originals.text_dependencies['jp/encoder'], path.join(root, 'text/encoder'));
  assert.ok(profile.binding.engine.files['next_launcher.py'].endsWith('/workers/sbv2/launch.py'));
  const output = path.join(root, 'candidate.json');
  await writeCandidate(output, result);
  assert.deepEqual(await readJson(output), result);
  await assert.rejects(writeCandidate(output, { changed: true }), /EEXIST/);
  assert.deepEqual(await readJson(output), result);
  assert.equal(await readFile(input.model, 'utf8'), 'fixture');
});
test('invalid inputs, missing originals and ambiguous catalog stop candidate generation', async t => {
  const { input, catalog } = await fixture(t);
  for (const changed of [{ ...input, extra: true }, { ...input, cpu_only: 'false' }, { ...input, endpoint: 'http://example.com:5050' }, { ...input, model: '/missing/model' }, { ...input, model: '/a/../model' }]) await assert.rejects(buildProfile(changed));
  await writeFile(input.catalog, JSON.stringify({ 0: catalog[0], 1: catalog[0] }));
  await assert.rejects(buildProfile(input), /一意/);
});
test('tree manifest includes Python bytecode, resolves file links and rejects cycles and truncation', async t => {
  const { root } = await fixture(t), tree = path.join(root, 'runtime');
  await mkdir(path.join(tree, '__pycache__'));
  await writeFile(path.join(tree, '__pycache__/code.pyc'), 'compiled');
  await symlink(path.join(root, 'text/encoder'), path.join(tree, 'linked'));
  const limits = { count: 10, perFile: 100, total: 1000, allowEmpty: true };
  const result = await manifest({ runtime: tree }, limits);
  assert.ok(result['runtime/__pycache__/code.pyc']);
  assert.equal(result['runtime/linked'], path.join(root, 'text/encoder'));
  await assert.rejects(manifest({ runtime: tree }, { ...limits, count: 1 }), /上限/);
  await assert.rejects(manifest({ runtime: tree }, { ...limits, total: 1 }), /上限/);
  await symlink(tree, path.join(tree, 'cycle'));
  await assert.rejects(manifest({ runtime: tree }, limits), /循環/);
});
test('special originals and output symlinks are not accepted', async t => {
  const { input, root } = await fixture(t);
  const config = path.join(path.dirname(input.model), 'config.json');
  await rm(config); await symlink(path.join(root, 'text/encoder'), config);
  await assert.rejects(buildProfile(input), /末端リンク/);
  const output = path.join(root, 'output.json');
  await symlink(path.join(root, 'missing.json'), output);
  await assert.rejects(writeCandidate(output, {}), /EEXIST/);
});
test('CPU setup can build its catalog offline while GPU mode and unselected originals are refused', async t => {
  const { input } = await fixture(t);
  const config = path.join(path.dirname(input.model), 'config.json');
  await writeFile(config, JSON.stringify({ version: '2.7.0-JP-Extra', data: { sampling_rate: 44100, num_styles: 1, spk2id: { Voice: 0 }, style2id: { Neutral: 0 } } }));
  const result = await buildProfile({ ...input, catalog: null });
  assert.equal(result.profiles[0].catalog[0].model_path, input.model);
  assert.equal(result.profiles[0].catalog[0].device, 'cpu');
  await assert.rejects(buildProfile({ ...input, catalog: null, cpu_only: false }), /CPU/);
  const newer = path.join(path.dirname(input.model), 'later.pt');
  await writeFile(newer, 'newer');
  const { utimes } = await import('node:fs/promises');
  await utimes(input.model, 100, 100); await utimes(newer, 200, 200);
  await assert.rejects(buildProfile({ ...input, catalog: null }), /一意/);
});

test('work directory must be separate from original trees including resolved links', async t => {
  const { root, input } = await fixture(t);
  for (const work_root of [input.engine_root, root, path.join(root, 'text')]) {
    await assert.rejects(buildProfile({ ...input, work_root }));
  }
  const alias = path.join(root, 'work-alias');
  await symlink(input.engine_root, alias);
  await assert.rejects(buildProfile({ ...input, work_root: alias }));
});
