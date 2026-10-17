import { readdir, readFile } from 'node:fs/promises';
import { basename, extname, join } from 'node:path';

const ignored = new Set(['.git', 'node_modules', 'target', 'dist']);
// Exact generated file paths only. Do not exempt entire source directories.
const generated = new Set(['packages/api-client/src/generated.ts']);
let failed = false;
let checked = 0;
async function walk(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory() && !ignored.has(entry.name)) await walk(path);
    if (!entry.isFile() || generated.has(path)) continue;
    if (!['.rs', '.ts', '.svelte', '.mjs', '.py'].includes(extname(path))) continue;
    checked++;
    const lines = (await readFile(path, 'utf8')).trimEnd().split('\n').length;
    const limit = ['main.rs', 'lib.rs', 'App.svelte'].includes(basename(path))
      ? 100 : extname(path) === '.svelte' ? 250 : 300;
    if (lines > limit) console.warn(`${path}: ${lines}行（目安${limit}）`);
    if (lines > 400) failed = true;
  }
}
await walk('.');
console.log(`ソースサイズ: ${checked}ファイル ${failed ? '不合格' : '合格'}`);
if (failed) process.exitCode = 1;
