import { test } from 'node:test';
import assert from 'node:assert/strict';
import { parseGlb, inspectKind } from '../../workers/vrm/src/glb.ts';

function glb(data: unknown, bin?: Buffer) {
  const json = Buffer.from(JSON.stringify(data));
  const padded = Buffer.alloc(Math.ceil(json.length / 4) * 4, 32); json.copy(padded);
  const chunks = [chunk(0x4e4f534a, padded), ...(bin ? [chunk(0x004e4942, bin)] : [])];
  const header = Buffer.alloc(12);
  header.writeUInt32LE(0x46546c67); header.writeUInt32LE(2, 4);
  header.writeUInt32LE(12 + chunks.reduce((n, b) => n + b.length, 0), 8);
  return Buffer.concat([header, ...chunks]);
}
function chunk(kind: number, bytes: Buffer) {
  const head = Buffer.alloc(8); head.writeUInt32LE(bytes.length); head.writeUInt32LE(kind, 4);
  return Buffer.concat([head, bytes]);
}
const asset = { version: '2.0' };

test('GLB chunk boundaries, BIN size, UTF8 and external references', () => {
  assert.equal(parseGlb(glb({ asset, extras: '日本語' })).extras, '日本語');
  parseGlb(glb({ asset, buffers: [{ byteLength: 1 }] }, Buffer.alloc(4)));
  for (const length of [0, 5, -1, 1.5]) {
    assert.throws(() => parseGlb(glb({ asset, buffers: [{ byteLength: length }] }, Buffer.alloc(4))));
  }
  assert.throws(() => parseGlb(glb({ asset, buffers: [{ byteLength: 4 }] })), /BIN/);
  assert.throws(() => parseGlb(glb({ asset, images: [{ uri: 42 }] })), /外部参照/);
  assert.throws(() => parseGlb(glb({ asset, images: {} })), /配列/);
  const truncated = glb({ asset }); truncated.writeUInt32LE(0xffffffff, 12);
  assert.throws(() => parseGlb(truncated), /chunk長/);
  const trailing = Buffer.concat([glb({ asset }), Buffer.from([0])]); trailing.writeUInt32LE(trailing.length, 8);
  assert.throws(() => parseGlb(trailing), /ヘッダー/);
  const duplicated = Buffer.concat([glb({ asset }), glb({ asset }).subarray(12)]); duplicated.writeUInt32LE(duplicated.length, 8);
  assert.throws(() => parseGlb(duplicated), /重複/);
});

test('model and motion distinction rejects generic GLB, swapped roles and unsupported versions', () => {
  const model = parseGlb(glb({ asset, extensions: { VRMC_vrm: {
    specVersion: '1.0', humanoid: {}, expressions: { preset: { aa: {} }, custom: { smile: {} } },
  } } }));
  assert.deepEqual(inspectKind(model, 'model'), { kind: 'model', version: '1.0', expressions: ['aa', 'smile'] });
  assert.throws(() => inspectKind(model, 'motion'), /VRMA/);
  const motion = parseGlb(glb({ asset, animations: [{}], extensions: { VRMC_vrm_animation: { specVersion: '1.0', humanoid: {} } } }));
  assert.equal(inspectKind(motion, 'motion').kind, 'motion');
  assert.throws(() => inspectKind(motion, 'model'), /モデル/);
  assert.throws(() => inspectKind(parseGlb(glb({ asset })), 'model'), /モデル/);
  model.extensions.VRMC_vrm.specVersion = '2.0';
  assert.throws(() => inspectKind(model, 'model'), /モデル/);
  motion.animations.push({});
  assert.throws(() => inspectKind(motion, 'motion'), /単一/);
});
