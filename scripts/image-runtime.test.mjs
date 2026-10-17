import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, access, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';

test('missing or wrong uv stops before creating runtime folders or installing anything', async () => {
  const bin = await mkdtemp(join(tmpdir(), 'vvm-uv-preflight-'));
  const mount = `/Volumes/vvm-uv-preflight-${process.pid}-${Date.now()}`;
  const run = () => spawnSync(process.execPath, ['scripts/image-runtime.mjs', 'setup'], {
    cwd: new URL('../', import.meta.url), encoding: 'utf8', timeout: 10000,
    env: { ...process.env, PATH: bin, SUNGEKI_RUNTIME_ROOT: `${mount}/runtime` },
  });
  try {
    const missing = run();
    assert.ifError(missing.error);
    assert.equal(missing.status, 1);
    assert.match(missing.stderr, /uv 0.10.6が見つかりません/);
    assert.doesNotMatch(missing.stderr, /at main|node:internal/);
    await writeFile(join(bin, 'uv'), '#!/bin/sh\n[ "$1" = "--version" ] || exit 99\nprintf "uv 0.0.1\\n"\n', { mode: 0o700 });
    const wrong = run();
    assert.ifError(wrong.error);
    assert.equal(wrong.status, 1);
    assert.match(wrong.stderr, /検出: 0.0.1/);
    assert.match(wrong.stderr, /指定版へ切り替え/);
    await access(mount).then(() => assert.fail('preflight created a mount folder'), error => assert.equal(error.code, 'ENOENT'));
  } finally { await rm(bin, { recursive: true, force: true }); }
});
