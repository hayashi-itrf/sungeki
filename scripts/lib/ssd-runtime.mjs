import { mkdir, realpath, stat } from 'node:fs/promises';
import { dirname, isAbsolute, resolve, sep } from 'node:path';

export function runtimeLocation(configured) {
  if (configured == null) throw new Error('SUNGEKI_RUNTIME_ROOTに外部ディスク内の専用保存フォルダを指定してください。準備コマンドはアプリの保存済み設定を自動では読み込みません');
  const value = configured;
  if (!value || !isAbsolute(value) || value.split(sep).includes('..')) throw new Error('Sungeki runtimeは外部ディスク内の絶対パスで指定してください');
  const root = resolve(value);
  const parts = root.split(sep);
  if (parts[1] !== 'Volumes' || !parts[2] || parts.length < 4) {
    throw new Error('Sungeki runtimeは /Volumes/ディスク名/専用フォルダー 配下へ指定してください');
  }
  return { root, mount: `/Volumes/${parts[2]}` };
}

export async function ssdRuntime(configured = process.env.SUNGEKI_RUNTIME_ROOT) {
  const { root, mount } = runtimeLocation(configured);
  const mounted = await stat(mount).catch(error => {
    if (error.code === 'ENOENT') throw new Error(`外部ディスクが未接続です: ${mount}`, { cause: error });
    throw error;
  });
  if (!mounted.isDirectory() || mounted.dev === (await stat('/Volumes')).dev) throw new Error('外部ディスクが未接続です');
  const actualMount = await realpath(mount);
  // Check the existing ancestor before mkdir, so a link cannot create folders
  // on the internal disk before the final containment check rejects it.
  let ancestor = root;
  while (true) {
    try { await stat(ancestor); break; }
    catch (error) {
      if (error.code !== 'ENOENT') throw error;
      const parent = dirname(ancestor);
      if (parent === ancestor) throw error;
      ancestor = parent;
    }
  }
  const existing = await realpath(ancestor);
  if (existing !== actualMount && !existing.startsWith(actualMount + sep)) throw new Error('runtime実体が外部ディスク外です');
  await mkdir(root, { recursive: true });
  const actual = await realpath(root);
  if (!actual.startsWith(actualMount + sep)) throw new Error('runtime実体が外部ディスク外です');
  return actual;
}
