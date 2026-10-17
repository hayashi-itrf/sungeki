// Mirrors the inspected vendor holder's directory and newest-file selection.
// Never imports Python, loads tensor weights, or claims a running model exists.
import { readdir, lstat } from 'node:fs/promises';
import path from 'node:path';
import { absolute, regular } from './manifest.mjs';
import { readJson } from './json.mjs';
const suffixes = new Set(['.pth', '.pt', '.safetensors', '.onnx']);
function pythonOrder(a, b) {
  const left = Array.from(a, c => c.codePointAt(0)), right = Array.from(b, c => c.codePointAt(0));
  for (let i = 0; i < Math.min(left.length, right.length); i++) if (left[i] !== right[i]) return left[i] - right[i];
  return left.length - right.length;
}
function names(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('話者/スタイル一覧が不正です。');
  const entries = Object.entries(value);
  if (!entries.length || entries.length > 4096 || new Set(entries.map(([, id]) => id)).size !== entries.length || entries.some(([name, id]) => !name.trim() || Buffer.byteLength(name) > 1024 || /[\x00-\x1f\x7f]/.test(name) || !Number.isInteger(id) || id < 0 || id > 0xffffffff)) throw new Error('話者/スタイルの名前またはIDが不正です。');
  return value;
}
export async function offlineCatalog(root) {
  root = absolute(root);
  const catalog = {};
  const directories = await readdir(root, { withFileTypes: true });
  if (directories.length > 10000) throw new Error('モデルフォルダの項目数が上限を超えています。');
  for (const entry of directories.sort((a, b) => pythonOrder(a.name, b.name))) {
    if (entry.name.startsWith('.')) continue;
    if (entry.isSymbolicLink()) throw new Error('モデルフォルダ内のリンクは自動一覧作成では扱えません。');
    if (!entry.isDirectory()) continue;
    const directory = path.join(root, entry.name);
    const entries = await readdir(directory, { withFileTypes: true });
    if (entries.length > 10000) throw new Error('モデルファイルの項目数が上限を超えています。');
    const models = [];
    for (const file of entries) {
      if (file.name.startsWith('.') || !suffixes.has(path.extname(file.name))) continue;
      if (!file.isFile()) throw new Error('モデル原本は通常ファイルで指定してください。');
      const target = path.join(directory, file.name);
      await regular(target, 8 * 1024 ** 3);
      models.push({ path: target, mtime: (await lstat(target)).mtimeMs });
    }
    if (!models.length || !entries.some(file => file.name === 'config.json')) continue;
    models.sort((a, b) => b.mtime - a.mtime);
    if (models.length > 1 && models[0].mtime === models[1].mtime) throw new Error('最新モデルが同じ更新日時で複数あります。選択を確定できません。');
    const configPath = path.join(directory, 'config.json'), stylesPath = path.join(directory, 'style_vectors.npy');
    for (const file of [configPath, stylesPath]) if (!(await lstat(file)).isFile()) throw new Error('モデル同伴ファイルは通常ファイルが必要です。');
    await regular(stylesPath, 256 * 1024 ** 2);
    const config = await readJson(configPath), data = config.data;
    if (!data || typeof config.version !== 'string' || !config.version.trim() || Buffer.byteLength(config.version) > 128 || /[\x00-\x1f\x7f]/.test(config.version) || !Number.isInteger(data.sampling_rate) || data.sampling_rate < 8000 || data.sampling_rate > 192000) throw new Error('モデルconfigの版または音声形式が不正です。');
    const spk2id = names(data.spk2id), style2id = names(data.style2id);
    if (data.num_styles !== Object.keys(style2id).length || Object.values(style2id).some(id => id >= data.num_styles)) throw new Error('スタイル数とIDが一致しません。');
    const id = Object.keys(catalog).length;
    if (id >= 4096) throw new Error('モデル数が上限を超えています。');
    catalog[id] = { config_path: configPath, model_path: models[0].path, device: 'cpu', spk2id,
      id2spk: Object.fromEntries(Object.entries(spk2id).map(([name, id]) => [id, name])), style2id };
  }
  if (!Object.keys(catalog).length) throw new Error('利用可能なモデル原本がありません。');
  return catalog;
}
