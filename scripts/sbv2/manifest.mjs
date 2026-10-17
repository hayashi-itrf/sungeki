import { lstat, realpath, readdir, stat } from 'node:fs/promises';
import path from 'node:path';

export function absolute(value) {
  if (typeof value !== 'string' || !path.isAbsolute(value) || value.split(path.sep).includes('..')) throw new Error('親参照を含まない絶対パスを指定してください。');
  return path.normalize(value);
}
export async function regular(value, maxBytes, allowEmpty = false) {
  const target = await realpath(absolute(value));
  const info = await lstat(target);
  if (!info.isFile() || (!allowEmpty && info.size === 0) || info.size > maxBytes) throw new Error(`通常ファイルの容量が不正です: ${target}`);
  return { target, size: info.size };
}
// Enumerate explicitly selected trees only. No imports, subprocesses or network.
// Include bytecode: Python -B disables writes, but can still read cached code.
export async function manifest(roots, { count, perFile, total, allowEmpty }) {
  if (!roots || typeof roots !== 'object' || Array.isArray(roots) || !Object.keys(roots).length) throw new Error('依存フォルダ一覧が必要です。');
  const files = {}, active = new Set();
  let bytes = 0, visited = 0, entries = 0;
  async function walk(source, logical) {
    if (++visited > 100000) throw new Error('フォルダ内の項目数が上限を超えています。');
    if (Buffer.byteLength(logical) > 1024 || /[\x00-\x1f\x7f]/.test(logical)) throw new Error('依存ファイル名が不正です。');
    const target = await realpath(source), info = await stat(target);
    if (info.isDirectory()) {
      if (active.has(target)) throw new Error(`循環したフォルダ参照です: ${source}`);
      active.add(target);
      for (const name of (await readdir(target)).sort()) {
        if (name === '.git') continue;
        await walk(path.join(target, name), `${logical}/${name}`);
      }
      active.delete(target);
    } else {
      const file = await regular(target, perFile, allowEmpty);
      if (Object.hasOwn(files, logical)) throw new Error('依存ファイル名が重複しています。');
      files[logical] = file.target; bytes += file.size;
      if (++entries > count || bytes > total) throw new Error('依存一覧がhostの件数または容量上限を超えています。対象を黙って省略できません。');
    }
  }
  for (const [name, source] of Object.entries(roots).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)) {
    if (!/^[A-Za-z0-9_-]{1,64}$/.test(name)) throw new Error('依存フォルダには英数字の名前を付けてください。');
    await walk(absolute(source), name);
  }
  if (!Object.keys(files).length) throw new Error('依存一覧が空です。');
  return files;
}
