import { open } from 'node:fs/promises';
import { regular } from './manifest.mjs';
const MiB = 1024 ** 2;
export async function readJson(file) {
  const checked = await regular(file, 4 * MiB);
  const handle = await open(checked.target, 'r');
  try {
    const buffer = Buffer.alloc(4 * MiB + 1);
    let used = 0;
    while (used < buffer.length) { const { bytesRead } = await handle.read(buffer, used, buffer.length - used, null); if (!bytesRead) break; used += bytesRead; }
    if (used > 4 * MiB) throw new Error('設定入力が4MiBを超えています。');
    return JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(buffer.subarray(0, used)));
  } finally { await handle.close(); }
}
