import { open } from 'node:fs/promises';
import { isAbsolute } from 'node:path';

export async function readSnapshot(path: string, minBytes: number, maxBytes: number) {
  if (!isAbsolute(path)) throw new Error('素材は絶対パスで指定してください');
  const file = await open(path, 'r');
  let bytes: Buffer;
  try {
    const before = await file.stat({ bigint: true });
    if (!before.isFile() || before.size < BigInt(minBytes) || before.size > BigInt(maxBytes)) {
      throw new Error('素材サイズが範囲外です');
    }
    bytes = Buffer.alloc(Number(before.size));
    let offset = 0;
    while (offset < bytes.length) {
      const read = await file.read(bytes, offset, bytes.length - offset, offset);
      if (!read.bytesRead) throw new Error('検査中に素材が短くなりました');
      offset += read.bytesRead;
    }
    const after = await file.stat({ bigint: true });
    if (before.size !== after.size || before.mtimeNs !== after.mtimeNs || before.ctimeNs !== after.ctimeNs) {
      throw new Error('検査中に素材が変更されました');
    }
  } finally { await file.close(); }
  return bytes;
}
