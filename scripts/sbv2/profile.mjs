import { open, realpath, lstat, link, unlink } from 'node:fs/promises';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { absolute, regular, manifest } from './manifest.mjs';
import { readJson } from './json.mjs';
import { offlineCatalog } from './catalog.mjs';
export { readJson } from './json.mjs';
const MiB = 1024 ** 2, GiB = 1024 ** 3;
export async function buildProfile(input) {
  const fields = ['id', 'name', 'endpoint', 'cpu_only', 'engine_root', 'work_root', 'python', 'model', 'catalog', 'engine_roots', 'text_roots'];
  if (!input || Array.isArray(input) || typeof input !== 'object' || Object.keys(input).length !== fields.length || Object.keys(input).some(k => !fields.includes(k))) throw new Error('設定入力の項目が不足しているか、未知の項目があります。');
  if (!/^[A-Za-z0-9_-]{1,64}$/.test(input.id) || typeof input.id !== 'string' || typeof input.name !== 'string' || !input.name.trim() || Buffer.byteLength(input.name) > 256 || /[\x00-\x1f\x7f]/.test(input.name) || typeof input.cpu_only !== 'boolean') throw new Error('設定ID・名前・CPU指定が不正です。');
  if (typeof input.endpoint !== 'string' || !/^http:\/\/127\.0\.0\.1:([0-9]+)\/?$/.test(input.endpoint)) throw new Error('接続先は127.0.0.1のHTTPポートで指定してください。');
  const port = Number(new URL(input.endpoint).port);
  if (!Number.isInteger(port) || port < 1024 || port > 65535) throw new Error('ポートは1024〜65535で指定してください。');
  const cwd = await realpath(absolute(input.engine_root)), model = await realpath(absolute(input.model));
  await regular(model, 8 * GiB);
  const work = await realpath(absolute(input.work_root));
  if (!(await lstat(work)).isDirectory() || work === cwd || work.startsWith(cwd + path.sep) || cwd.startsWith(work + path.sep)) throw new Error('作業先をエンジン原本から分離してください。');
  const config = path.join(path.dirname(model), 'config.json'), styles = path.join(path.dirname(model), 'style_vectors.npy');
  for (const file of [config, styles]) if (!(await lstat(file)).isFile()) throw new Error('config/style_vectorsの末端リンクや特殊ファイルは使えません。');
  await regular(config, 4 * MiB); await regular(styles, 256 * MiB);
  const executable = absolute(input.python), target = await realpath(executable);
  await regular(target, 512 * MiB);
  const engineFiles = await manifest(input.engine_roots, { count: 65532, perFile: 512 * MiB, total: 8 * GiB, allowEmpty: true });
  for (const name of ['server_fastapi.py', 'config.py', 'config.yml']) {
    const file = path.join(cwd, name);
    if (!(await lstat(file)).isFile()) throw new Error('起動ファイルの末端リンクや特殊ファイルは使えません。');
    engineFiles[name] = (await regular(file, 512 * MiB, true)).target;
  }
  engineFiles['next_launcher.py'] = fileURLToPath(new URL('../../workers/sbv2/launch.py', import.meta.url));
  let engineBytes = 0;
  for (const file of Object.values(engineFiles)) engineBytes += (await regular(file, 512 * MiB, true)).size;
  if (engineBytes > 8 * GiB) throw new Error('エンジン依存の総容量が上限を超えています。');
  const dependencies = await manifest(input.text_roots, { count: 128, perFile: 8 * GiB, total: 32 * GiB, allowEmpty: false });
  // Offline CPU metadata is a prediction, rechecked against the owned server.
  if (input.catalog === null && !input.cpu_only) throw new Error('自動一覧作成はCPU指定が必要です。GPUのdeviceを推測しません。');
  if ([model, config, styles, target, ...Object.values(engineFiles), ...Object.values(dependencies)].some(file => file === work || file.startsWith(work + path.sep) || work.startsWith(path.dirname(file) + path.sep))) throw new Error('作業先に原本を配置できません。');
  const catalog = input.catalog === null ? await offlineCatalog(path.dirname(path.dirname(model))) : await readJson(input.catalog);
  if (!catalog || typeof catalog !== 'object' || Array.isArray(catalog)) throw new Error('catalogはモデルIDをキーとするオブジェクトが必要です。');
  const candidates = Object.values(catalog).filter(m => m && path.resolve(cwd, m.model_path ?? '') === model && path.resolve(cwd, m.config_path ?? '') === config);
  if (candidates.length !== 1) throw new Error('catalogに選択した原本が一意に含まれていません。');
  return { format_version: 1, profiles: [{ id: input.id, name: input.name, cpu_only: input.cpu_only, catalog,
    binding: { endpoint: input.endpoint, working_directory: cwd, work_root: work, originals: { model, config, style_vectors: styles, text_dependencies: dependencies }, engine: { executable, executable_target: target, files: engineFiles } } }] };
}
export async function writeCandidate(file, configuration) {
  const body = JSON.stringify(configuration, null, 2) + '\n';
  if (Buffer.byteLength(body) > 32 * MiB) throw new Error('設定候補が32MiBを超えています。');
  const destination = absolute(file), temporary = path.join(path.dirname(destination), `.sbv2-candidate-${randomUUID()}.tmp`);
  const handle = await open(temporary, 'wx', 0o600);
  try {
    try { await handle.writeFile(body); await handle.sync(); } finally { await handle.close(); }
    await link(temporary, destination); // Atomic publication; never replace an existing file or symlink.
  } finally { await unlink(temporary); }
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    if (process.argv.length !== 4) throw new Error('使い方: node scripts/sbv2/profile.mjs /絶対パス/setup.json /絶対パス/sbv2.candidate.json');
    await writeCandidate(process.argv[3], await buildProfile(await readJson(process.argv[2])));
    console.log('設定候補を作成しました。未導入・未合成です。依存一覧の完全性とhostでの原本検査が必要です。');
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
