import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';

test('metadata CLI returns one versioned JSON response and rejects swapped roles without success output', async () => {
  const root = await mkdtemp(join(tmpdir(), 'vvm-vrm-cli-'));
  try {
    const path = join(root, 'model.vrm');
    const json = Buffer.from(JSON.stringify({ asset: { version: '2.0' }, extensions: {
      VRMC_vrm: { specVersion: '1.0', humanoid: {}, expressions: { preset: { aa: {} } } },
    } }));
    const padded = Buffer.alloc(Math.ceil(json.length / 4) * 4, 32); json.copy(padded);
    const header = Buffer.alloc(20);
    [0x46546c67, 2, 20+padded.length, padded.length, 0x4e4f534a].forEach((n,i) => header.writeUInt32LE(n,i*4));
    const bytes = Buffer.concat([header, padded]); await writeFile(path, bytes);
    const run = (kind: string) => spawnSync(process.execPath, ['--experimental-strip-types',
      'workers/vrm/src/inspect.ts', '--path', path, '--kind', kind], { encoding: 'utf8', timeout: 10000 });
    const good = run('model'); assert.equal(good.status, 0, good.stderr);
    const data = JSON.parse(good.stdout);
    assert.equal(data.version, 1); assert.equal(data.bytes, bytes.length);
    assert.equal(data.sha256, createHash('sha256').update(bytes).digest('hex'));
    assert.deepEqual(data.inspection, { kind: 'model', version: '1.0', expressions: ['aa'] });
    for (const kind of ['motion', 'unknown']) {
      const bad = run(kind); assert.equal(bad.status, 1); assert.equal(bad.stdout, '');
    }
  } finally { await rm(root, { recursive: true, force: true }); }
});
