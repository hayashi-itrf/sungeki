import { access, readFile, readdir, stat } from 'node:fs/promises';
import { constants } from 'node:fs';
import { createHash } from 'node:crypto';
import { isAbsolute, join } from 'node:path';

// Construct child-process settings; never mutate the running process or install dependencies.
export async function desktopEnvironment(root, runtime, inherited = process.env, node = process.execPath, unavailable = () => {}) {
  const executables = new Set(['SUNGEKI_RENDER_PYTHON', 'SUNGEKI_RENDER_FFMPEG', 'SUNGEKI_RENDER_FFPROBE', 'SUNGEKI_VRM_NODE']);
  const file = async (name, fallback) => {
    const path = inherited[name] ?? fallback;
    if (!path || !isAbsolute(path) || !(await stat(path).catch(() => null))?.isFile()) {
      if (inherited[name] !== undefined) throw Error(`${name} のファイルが見つかりません。実行環境を準備してください。`);
      unavailable(`${name} は未導入です。この機能を使う前に実行環境を準備してください。`);
      return undefined;
    }
    const mode = executables.has(name) ? constants.X_OK : constants.R_OK;
    if (!await access(path, mode).then(() => true, () => false)) {
      const action = executables.has(name) ? '実行' : '読み取り';
      const message = `${name} のファイルを${action}できません。利用できるファイルを指定し直してください。`;
      if (inherited[name] !== undefined) throw Error(message);
      unavailable(message);
      return undefined;
    }
    return path;
  };
  let font = inherited.SUNGEKI_RENDER_FONT;
  if (!font) {
    const directory = '/System/Library/Fonts';
    const name = (await readdir(directory).catch(() => [])).find(n => n.normalize('NFC') === 'ヒラギノ角ゴシック W3.ttc');
    if (name) font = join(directory, name);
  }
  const paths = {
    SUNGEKI_RENDER_PYTHON: join(runtime, 'image/venv/bin/python'),
    SUNGEKI_RENDER_WORKER: join(root, 'workers/image/render_frame.py'),
    SUNGEKI_RENDER_FFMPEG: '/opt/homebrew/bin/ffmpeg',
    SUNGEKI_RENDER_FFPROBE: '/opt/homebrew/bin/ffprobe',
    SUNGEKI_RENDER_FONT: font,
    SUNGEKI_VRM_NODE: node,
    SUNGEKI_VRM_INSPECT_WORKER: join(root, 'workers/vrm/src/inspect.ts'),
    SUNGEKI_VRM_RENDER_WORKER: join(root, 'workers/vrm/src/render-main.ts'),
  };
  const env = { ...inherited, SUNGEKI_RUNTIME_ROOT: runtime };
  for (const [name, fallback] of Object.entries(paths)) {
    const path = await file(name, fallback);
    if (path !== undefined) env[name] = path;
  }
  if (!env.SUNGEKI_RENDER_FONT) {
    if (inherited.SUNGEKI_RENDER_FONT_SHA256) throw Error('フォントの検査値にはフォントの設定が必要です。');
    return env;
  }
  const hash = createHash('sha256').update(await readFile(env.SUNGEKI_RENDER_FONT)).digest('hex');
  if (inherited.SUNGEKI_RENDER_FONT_SHA256 && inherited.SUNGEKI_RENDER_FONT_SHA256 !== hash)
    throw Error('フォントの検査値が設定と一致しません。');
  env.SUNGEKI_RENDER_FONT_SHA256 = hash;
  return env;
}
