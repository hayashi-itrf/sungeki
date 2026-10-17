import { expect, test } from 'vitest';
import { readVrmBytes } from './vrm-bytes';
import type { Asset, SessionResponse } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/vrm-clip-project.json';
const bytes = new Uint8Array(25).fill(3);
async function source() {
  const hash = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes)), b => b.toString(16).padStart(2,'0')).join('');
  const asset = structuredClone(fixture.assets[0]) as Asset;
  if (asset.probe.type !== 'vrm') throw Error();
  asset.probe.info.byte_length = bytes.length; asset.probe.info.sha256 = hash;
  const response: SessionResponse = { type:'vrm_asset_bytes', offset:0, total:bytes.length, sha256:hash, base64:btoa(String.fromCharCode(...bytes)) };
  return { asset, response };
}
test('registered model bytes are digest verified before loading', async () => {
  const { asset, response } = await source();
  expect(new Uint8Array(await readVrmBytes(async () => response,'session',7,asset,() => false))).toEqual(bytes);
});
test('corrupted bytes and stale ranges never reach model loader', async () => {
  const { asset, response } = await source();
  if (response.type !== 'vrm_asset_bytes') throw Error();
  await expect(readVrmBytes(async () => ({...response, base64:btoa('x'.repeat(25))}),'s',7,asset,() => false)).rejects.toThrow('変更');
  await expect(readVrmBytes(async () => ({...response, offset:1}),'s',7,asset,() => false)).rejects.toThrow('一致');
  await expect(readVrmBytes(async () => response,'s',7,asset,() => true)).rejects.toThrow('中止');
});
