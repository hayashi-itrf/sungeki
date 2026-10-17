import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const profile = process.argv.includes('--dist') ? 'dist' : 'release';
execFileSync('cargo', ['build', '--profile', profile, '--locked', '-p', 'sungeki-host'], { cwd: root, stdio: 'inherit' });
const version = execFileSync('rustc', ['-vV'], { cwd: root, encoding: 'utf8' });
const triple = version.match(/^host: (.+)$/m)?.[1];
if (!triple) throw new Error('Rustのターゲットを判定できません');
const directory = resolve(root, 'apps/desktop/src-tauri/binaries');
mkdirSync(directory, { recursive: true });
copyFileSync(resolve(root, `target/${profile}/sungeki-host`), resolve(directory, `sungeki-host-${triple}`));
console.log(`Sungeki host sidecarを配置しました (${profile}, ${triple})`);

await import('./prepare-image-worker.mjs');
