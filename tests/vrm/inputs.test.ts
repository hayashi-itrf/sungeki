import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { readGlb } from '../../workers/vrm/src/inputs.ts';

function glb(json: unknown) {
  const data = Buffer.from(JSON.stringify(json).padEnd(Math.ceil(JSON.stringify(json).length / 4) * 4));
  const bytes = Buffer.alloc(20 + data.length);
  [0x46546c67, 2, bytes.length, data.length, 0x4e4f534a].forEach((value, index) => bytes.writeUInt32LE(value, index * 4));
  data.copy(bytes, 20);
  return bytes;
}

test('GLB boundary: snapshot hash, external references, truncated input', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'vvm-glb-test-'));
  const path = join(directory, 'fixture.glb');
  try {
    await writeFile(path, glb({ asset: { version: '2.0' } }));
    const snapshot = await readGlb(path);
    assert.equal(snapshot.manifest.sha256.length, 64);
    await writeFile(path, glb({ asset: { version: '2.0' }, images: [{ uri: 'https://example.com/image.png' }] }));
    await assert.rejects(readGlb(path), /外部参照/);
    await writeFile(path, snapshot.bytes.subarray(0, 20));
    await assert.rejects(readGlb(path), /GLB/);
    await assert.rejects(readGlb('relative.vrm'), /絶対パス/);
  } finally { await rm(directory, { recursive: true, force: true }); }
});
