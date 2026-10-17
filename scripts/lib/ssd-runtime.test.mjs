import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, symlink, access, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runtimeLocation, ssdRuntime } from './ssd-runtime.mjs';
import { spawnSync } from 'node:child_process';

test('explicit runtime selects its own volume; an omitted root never selects a developer disk', () => {
  assert.deepEqual(runtimeLocation('/Volumes/動画用 SSD/Sungeki/runtime'), {
    root: '/Volumes/動画用 SSD/Sungeki/runtime', mount: '/Volumes/動画用 SSD',
  });
  assert.throws(() => runtimeLocation(), /SUNGEKI_RUNTIME_ROOT/);
  for (const path of ['', 'relative/path', '/tmp/next', '/Volumes', '/Volumes/SSD', '/Volumes/SSD/../../tmp/next', '/Volumes/SSD/a/../runtime']) {
    assert.throws(() => runtimeLocation(path));
  }
});

test('image and VRM setup stop before installing when storage was not selected', () => {
  const env = { ...process.env };
  delete env.SUNGEKI_RUNTIME_ROOT;
  for (const args of [
    ['scripts/image-runtime.mjs', 'setup'],
    ['--experimental-strip-types', 'workers/vrm/src/setup.ts'],
  ]) {
    const result = spawnSync(process.execPath, args, {
      cwd: new URL('../../', import.meta.url), env, encoding: 'utf8', timeout: 10000,
    });
    assert.ifError(result.error);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /SUNGEKI_RUNTIME_ROOT/);
  }
});

test('an absent volume is rejected without creating a mount directory', async () => {
  const mount = `/Volumes/vvm-absent-${process.pid}-${Date.now()}`;
  await assert.rejects(ssdRuntime(`${mount}/runtime`));
  await assert.rejects(access(mount), { code: 'ENOENT' });
});

test('existing external volume accepts dedicated paths but rejects symlink escape before mkdir', {
  skip: !process.env.SUNGEKI_RUNTIME_TEST_ROOT,
}, async () => {
  const root = await ssdRuntime(process.env.SUNGEKI_RUNTIME_TEST_ROOT);
  const owned = await mkdtemp(join(root, 'runtime-path-test-'));
  const outside = await mkdtemp(join(tmpdir(), 'runtime-outside-test-'));
  try {
    assert.equal(await ssdRuntime(join(owned, 'nested', 'runtime')), join(owned, 'nested', 'runtime'));
    await symlink(outside, join(owned, 'escape'));
    await assert.rejects(ssdRuntime(join(owned, 'escape', 'must-not-create')), /実体/);
    await assert.rejects(access(join(outside, 'must-not-create')), { code: 'ENOENT' });
    await mkdir(join(owned, 'valid'));
    await symlink(join(owned, 'valid'), join(owned, 'inside'));
    assert.equal(await ssdRuntime(join(owned, 'inside', 'runtime')), join(owned, 'valid', 'runtime'));
  } finally {
    await rm(owned, { recursive: true, force: true });
    await rm(outside, { recursive: true, force: true });
  }
});
