// Offline inventory only. No imports from the engine, model load or installation.
import { realpath, stat } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { absolute, manifest, regular } from '../sbv2/manifest.mjs';
import { readJson } from '../sbv2/json.mjs';
const GiB = 1024 ** 3;
async function directory(value) {
  const target = await realpath(absolute(value));
  if (!(await stat(target)).isDirectory()) throw new Error(`フォルダではありません: ${target}`);
  return target;
}
export async function buildSetup(input) {
  const keys = ['id', 'name', 'port', 'engine_root', 'model_root', 'voice_root', 'work_root', 'python', 'engine_roots', 'voice_roots', 'catalog', 'default_settings', 'sample_rate', 'channels'];
  if (!input || typeof input !== 'object' || Array.isArray(input) || Object.keys(input).some(k => !keys.includes(k))) throw new Error('Qwen準備入力の項目が不正です。');
  if (typeof input.id !== 'string' || !/^[A-Za-z0-9_-]{1,64}$/.test(input.id) || typeof input.name !== 'string' || !input.name.trim()
      || Buffer.byteLength(input.name) > 256 || /[\x00-\x1f\x7f]/.test(input.name)) throw new Error('設定ID・名前が不正です。');
  if (!Number.isInteger(input.port) || input.port < 1024 || input.port > 65535
      || !Number.isInteger(input.sample_rate) || input.sample_rate < 8000 || input.sample_rate > 192000
      || ![1, 2].includes(input.channels)) throw new Error('ポートまたは音声形式が不正です。');
  const roots = {};
  for (const name of ['engine', 'model', 'voice', 'work']) roots[`${name}_root`] = await directory(input[`${name}_root`]);
  for (const name of ['engine', 'model', 'voice']) {
    const relative = path.relative(roots[`${name}_root`], roots.work_root);
    if (!relative || (!relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative))) throw new Error('作業フォルダは原本から分離してください。');
  }
  const executable = absolute(input.python), executable_target = (await regular(executable, 512 * 1024 ** 2)).target;
  const launcher = fileURLToPath(new URL('../../workers/qwen/launch.py', import.meta.url));
  const engine = await manifest(input.engine_roots, { count: 65532, perFile: 8 * GiB, total: 8 * GiB, allowEmpty: true });
  for (const [name, file] of [
    ['api/main.py', path.join(roots.engine_root, 'api/main.py')],
    ['next_launcher.py', launcher], ['python_executable', executable_target],
    ['pyvenv.cfg', path.join(path.dirname(path.dirname(executable)), 'pyvenv.cfg')],
  ]) {
    const target = (await regular(file, 8 * GiB, true)).target;
    if (Object.hasOwn(engine, name) && engine[name] !== target) throw new Error(`予約された依存名が別の原本を指しています: ${name}`);
    engine[name] = target;
  }
  let total = 0;
  for (const file of Object.values(engine)) total += (await regular(file, 8 * GiB, true)).size;
  if (total > 8 * GiB) throw new Error('エンジン一覧の総容量が上限を超えています。');
  const voice = await manifest(input.voice_roots, { count: 65536, perFile: GiB, total: GiB, allowEmpty: false });
  const catalog = await readJson(input.catalog);
  if (!Array.isArray(catalog) || catalog.length !== 3) throw new Error('catalogにはhealth/models/voicesの3要素が必要です。');
  const setup = { format_version: 1, id: input.id, name: input.name, port: input.port,
    binding: { ...roots, executable, executable_target, launcher, originals: { model: {}, engine, voice } },
    catalog, default_settings: input.default_settings, sample_rate: input.sample_rate, channels: input.channels };
  if (Buffer.byteLength(JSON.stringify(setup)) > 32 * 1024 ** 2) throw new Error('設定作成入力が32MiBを超えています。');
  return setup;
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    if (process.argv.length !== 3) throw new Error('使い方: node scripts/qwen/setup.mjs /absolute/inventory.json');
    console.log(JSON.stringify(await buildSetup(await readJson(process.argv[2])), null, 2));
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
