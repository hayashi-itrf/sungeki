import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, rm, symlink, realpath } from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { buildSetup } from './setup.mjs';
async function fixture(t) {
  const root = await realpath(await mkdtemp(path.join(os.tmpdir(), 'qwen-inventory-')));
  t.after(() => rm(root, { recursive: true, force: true }));
  for (const name of ['engine/api', 'model', 'voice', 'work', 'venv/bin', 'runtime']) await mkdir(path.join(root, name), { recursive: true });
  for (const [name, body] of [['engine/api/main.py','main'], ['runtime/package.py',''], ['voice/mapping.json','{}'], ['venv/bin/python','binary'], ['venv/pyvenv.cfg','include-system-site-packages = false'], ['catalog.json','[{},{},{}]']]) await writeFile(path.join(root, name), body);
  return { root, input: { id: 'local', name: '日本語', port: 18880,
    ...Object.fromEntries(['engine','model','voice','work'].map(n => [`${n}_root`, path.join(root,n)])),
    python: path.join(root,'venv/bin/python'), engine_roots: { sources: path.join(root,'engine'), dependencies: path.join(root,'runtime') },
    voice_roots: { voices: path.join(root,'voice') }, catalog: path.join(root,'catalog.json'),
    default_settings: { model:'qwen3-tts', voice:'Vivian', language:'Japanese', speed:1, instruct:null }, sample_rate:24000, channels:1 } };
}
test('enumerates explicit engine and voice trees and leaves model discovery to host', async t => {
  const { input, root } = await fixture(t);
  const before = structuredClone(input), result = await buildSetup(input);
  assert.deepEqual(input, before);
  assert.deepEqual(result.binding.originals.model, {});
  assert.equal(result.binding.originals.engine['dependencies/package.py'], path.join(root,'runtime/package.py'));
  assert.equal(result.binding.originals.engine['api/main.py'], path.join(root,'engine/api/main.py'));
  assert.equal(Object.keys(result.binding.originals.engine).length, 6);
  assert.equal(result.binding.originals.voice['voices/mapping.json'], path.join(root,'voice/mapping.json'));
});
test('rejects overlap, unknown input, invalid format, empty voice and cyclic dependencies', async t => {
  const { input, root } = await fixture(t);
  for (const change of [{ work_root: input.model_root }, { command:'arbitrary' }, { id:123 }, { sample_rate:0 }, { port:80 }]) await assert.rejects(buildSetup({ ...input, ...change }));
  await writeFile(path.join(root,'voice/empty'), '');
  await assert.rejects(buildSetup(input));
  await rm(path.join(root,'voice/empty'));
  await symlink(path.join(root,'runtime'), path.join(root,'runtime/loop'));
  await assert.rejects(buildSetup(input), /循環/);
});

test('required entry already inventoried at its exact name is accepted, different original rejected', async t => {
  const { input, root } = await fixture(t);
  input.engine_roots.api = path.join(root, 'engine/api');
  const result = await buildSetup(input);
  assert.equal(result.binding.originals.engine['api/main.py'], path.join(root, 'engine/api/main.py'));
  await writeFile(path.join(root, 'runtime/main.py'), 'wrong entry');
  input.engine_roots.api = path.join(root, 'runtime');
  await assert.rejects(buildSetup(input), /別の原本/);
});
