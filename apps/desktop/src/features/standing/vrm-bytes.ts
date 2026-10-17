import type { Asset, SessionRequest, SessionResponse } from '@sungeki/api-client';
type Send = (request: SessionRequest) => Promise<SessionResponse>;
export async function readVrmBytes(send: Send, sessionId: string, revision: number, asset: Asset,
  cancelled: () => boolean): Promise<ArrayBuffer> {
  if (asset.probe.type !== 'vrm') throw Error('VRM素材がありません');
  const probe = asset.probe.info;
  if (probe.byte_length < 20 || probe.byte_length > 100 * 1024 * 1024) throw Error('VRM素材の容量が範囲外です');
  const result = new Uint8Array(probe.byte_length);
  let offset = 0;
  while (offset < result.length) {
    if (cancelled()) throw Error('読込を中止しました');
    const response = await send({ type: 'read_vrm_asset', session_id: sessionId,
      expected_revision: revision, asset_id: asset.id, offset, length: 4 * 1024 * 1024 });
    if (response.type !== 'vrm_asset_bytes' || response.offset !== offset || response.total !== result.length
      || response.sha256 !== probe.sha256) throw Error('VRM原本の応答が一致しません');
    const chunk = Uint8Array.from(atob(response.base64), c => c.charCodeAt(0));
    if (!chunk.length || chunk.length > 4 * 1024 * 1024 || offset + chunk.length > result.length) throw Error('VRM読込範囲が不正です');
    result.set(chunk, offset); offset += chunk.length;
  }
  if (cancelled()) throw Error('読込を中止しました');
  const digest = await crypto.subtle.digest('SHA-256', result);
  const hash = Array.from(new Uint8Array(digest), b => b.toString(16).padStart(2, '0')).join('');
  if (hash !== probe.sha256) throw Error('VRM原本が変更されています。素材を再登録してください');
  return result.buffer;
}
