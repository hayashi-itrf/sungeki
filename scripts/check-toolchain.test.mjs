import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';

test('missing npm explains setup without an internal stack trace', () => {
  const bin = mkdtempSync(join(tmpdir(), 'sungeki-toolchain-'));
  try {
    const result = spawnSync(process.execPath, ['scripts/check-toolchain.mjs'], {
      cwd: new URL('../', import.meta.url), encoding: 'utf8',
      env: { ...process.env, PATH: bin }, timeout: 15000,
    });
    assert.ifError(result.error);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /npmを起動できません/);
    assert.match(result.stderr, /docs\/INSTALLATION.md/);
    assert.doesNotMatch(result.stderr, /node:internal|at /);
  } finally { rmSync(bin, { recursive: true, force: true }); }
});

test('wrong npm points to installation instructions before dependency setup', () => {
  const bin = mkdtempSync(join(tmpdir(), 'sungeki-toolchain-'));
  try {
    writeFileSync(join(bin, 'npm'), '#!/bin/sh\nprintf "0.0.1\\n"\n', { mode: 0o700 });
    const result = spawnSync(process.execPath, ['scripts/check-toolchain.mjs'], {
      cwd: new URL('../', import.meta.url), encoding: 'utf8',
      env: { ...process.env, PATH: bin }, timeout: 15000,
    });
    assert.ifError(result.error);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /npm: 0.0.1/);
    assert.match(result.stderr, /docs\/INSTALLATION.md/);
  } finally { rmSync(bin, { recursive: true, force: true }); }
});
