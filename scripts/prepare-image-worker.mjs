// Stage only application source, never Python packages, caches, fixtures or models.
import { copyFileSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const source = resolve(root, 'workers/image');
const target = resolve(root, 'apps/desktop/src-tauri/resources/image-worker');
rmSync(target, { recursive: true, force: true });
mkdirSync(target, { recursive: true });
const manifest = {};
for (const file of readdirSync(source).sort()) {
  if (!(file.endsWith('.py') && !file.startsWith('test_')) && !['pyproject.toml', 'uv.lock'].includes(file)) continue;
  copyFileSync(resolve(source, file), resolve(target, file));
  manifest[file] = createHash('sha256').update(readFileSync(resolve(target, file))).digest('hex');
}
if (!manifest['render_frame.py'] || !manifest['render_batch.py']) throw Error('描画プログラムが不足しています');
writeFileSync(resolve(target, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n');
console.log(`画像処理プログラム${Object.keys(manifest).length}ファイルを同梱用に配置しました`);
