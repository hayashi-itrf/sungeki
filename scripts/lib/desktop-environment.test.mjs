import { test } from 'node:test';
import assert from 'node:assert/strict';
import { chmod, mkdtemp, writeFile, rm, symlink } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { desktopEnvironment } from './desktop-environment.mjs';
test('child configuration preserves overrides and rejects missing files or changed font', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'vvm-env-'));
  try {
    const path = join(directory, 'fixture'); await writeFile(path, 'fixture', { mode: 0o700 });
    const keys = ['RENDER_PYTHON','RENDER_WORKER','RENDER_FFMPEG','RENDER_FFPROBE','RENDER_FONT','VRM_NODE','VRM_INSPECT_WORKER','VRM_RENDER_WORKER'];
    const input = Object.fromEntries(keys.map(k => [`SUNGEKI_${k}`,path]));
    input.KEEP_ME = 'preserved';
    const before = structuredClone(input);
    const env = await desktopEnvironment('/unused', directory, input);
    assert.deepEqual(input,before); assert.equal(env.KEEP_ME,'preserved');
    assert.equal(env.SUNGEKI_VRM_NODE,path); assert.equal(env.SUNGEKI_RUNTIME_ROOT,directory);
    assert.match(env.SUNGEKI_RENDER_FONT_SHA256,/^[a-f0-9]{64}$/);
    await assert.rejects(desktopEnvironment('/unused',directory,{...input,SUNGEKI_VRM_NODE:'/missing'}),/VRM_NODE/);
    await assert.rejects(desktopEnvironment('/unused',directory,{...input,SUNGEKI_RENDER_FONT_SHA256:'0'.repeat(64)}),/フォント/);
  } finally { await rm(directory,{recursive:true,force:true}); }
});

test('executable settings reject plain files, optional tools warn, and venv links retain their path', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'vvm-env-permissions-'));
  try {
    const plain = join(directory, 'not-executable');
    await writeFile(plain, 'fixture', { mode: 0o600 });
    for (const name of ['RENDER_PYTHON', 'RENDER_FFMPEG', 'RENDER_FFPROBE', 'VRM_NODE']) {
      await assert.rejects(desktopEnvironment('/missing-project', directory,
        { [`SUNGEKI_${name}`]: plain }), /実行できません/);
    }
    const warnings = [];
    const optional = await desktopEnvironment('/missing-project', directory, {}, plain, m => warnings.push(m));
    assert.equal(optional.SUNGEKI_VRM_NODE, undefined);
    assert.ok(warnings.some(m => m.includes('VRM_NODE') && m.includes('実行できません')));
    const python = join(directory, 'venv-python');
    await symlink(process.execPath, python);
    const env = await desktopEnvironment('/missing-project', directory, {
      SUNGEKI_RENDER_PYTHON: python, SUNGEKI_RENDER_WORKER: plain,
    });
    assert.equal(env.SUNGEKI_RENDER_PYTHON, python);
    assert.equal(env.SUNGEKI_RENDER_WORKER, plain);
    await chmod(plain, 0o700);
    const executable = await desktopEnvironment('/missing-project', directory, { SUNGEKI_VRM_NODE: plain });
    assert.equal(executable.SUNGEKI_VRM_NODE, plain);
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test('missing default workers do not prevent editing or discard available explicit settings', async () => {
  const warnings = [];
  const env = await desktopEnvironment('/missing-project', '/missing-runtime',
    { SUNGEKI_VRM_NODE: process.execPath }, process.execPath, message => warnings.push(message));
  assert.equal(env.SUNGEKI_VRM_NODE, process.execPath);
  assert.equal(env.SUNGEKI_RENDER_PYTHON, undefined);
  assert.equal(env.SUNGEKI_RENDER_WORKER, undefined);
  assert.equal(env.SUNGEKI_VRM_INSPECT_WORKER, undefined);
  assert.ok(warnings.some(message => message.includes('RENDER_PYTHON')));
  assert.ok(warnings.some(message => message.includes('VRM_INSPECT_WORKER')));
});
