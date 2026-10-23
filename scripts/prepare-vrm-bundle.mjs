// Build-time packaging only. End users never download a runtime on first use.
import { execFileSync } from 'node:child_process';
import { cpSync, mkdirSync, readFileSync, writeFileSync, existsSync, rmSync, readdirSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
if (process.platform !== 'darwin' || !['arm64', 'x64'].includes(process.arch)) throw Error('macOS build required');
const run = (cmd, args, options = {}) => execFileSync(cmd, args, { cwd: root, stdio: 'inherit', ...options });
const version = readFileSync(resolve(root, '.node-version'), 'utf8').trim().replace(/^v/, '');
const downloads = resolve(root, 'target/vrm-downloads');
const stage = resolve(root, 'apps/desktop/src-tauri/resources/vrm-runtime');
mkdirSync(downloads, { recursive: true });
const archive = `node-v${version}-darwin-${process.arch}.tar.gz`;
const base = `https://nodejs.org/dist/v${version}/`;
const sums = resolve(downloads, `node-${version}-SHASUMS256.txt`);
if (!existsSync(sums)) run('curl', ['--fail', '--location', '--retry', '2', base + 'SHASUMS256.txt', '-o', sums]);
const expected = readFileSync(sums, 'utf8').split('\n').find(line => line.endsWith('  ' + archive))?.split(' ')[0];
if (!expected) throw Error('Node checksum missing');
const tar = resolve(downloads, archive);
if (!existsSync(tar)) run('curl', ['--fail', '--location', '--retry', '2', base + archive, '-o', tar]);
if (createHash('sha256').update(readFileSync(tar)).digest('hex') !== expected) throw Error('Node archive checksum mismatch');
run('tar', ['-xzf', tar, '-C', downloads]);
const nodeRoot = resolve(downloads, archive.replace('.tar.gz', ''));
const descriptor = JSON.parse(readFileSync(resolve(root, 'node_modules/playwright-core/browsers.json')))
  .browsers.find(b => b.name === 'chromium-headless-shell');
const cache = process.env.SUNGEKI_BUNDLE_BROWSER_CACHE || resolve(downloads, 'playwright');
const browser = resolve(cache, `chromium_headless_shell-${descriptor.revision}`, `chrome-headless-shell-mac-${process.arch}`);
if (!existsSync(browser)) run(process.execPath, ['node_modules/playwright/cli.js', 'install', 'chromium', '--only-shell'],
  { env: { ...process.env, PLAYWRIGHT_BROWSERS_PATH: cache } });
const reported = execFileSync(resolve(browser, 'chrome-headless-shell'), ['--version'], { encoding: 'utf8' });
if (!reported.includes(descriptor.browserVersion)) throw Error('Browser version mismatch');
run('npm', ['run', 'build:vrm']);
rmSync(stage, { recursive: true, force: true });
mkdirSync(resolve(stage, 'bin'), { recursive: true });
const copy = (source, dest) => cpSync(source, resolve(stage, dest), { recursive: true, dereference: false });
copy(resolve(nodeRoot, 'bin/node'), 'bin/node');
copy(resolve(nodeRoot, 'LICENSE'), 'licenses/NODE-LICENSE.txt');
copy(browser, `browser/chrome-headless-shell-mac-${process.arch}`);
copy(resolve(browser, 'LICENSE.headless_shell'), 'licenses/CHROMIUM-LICENSE.txt');
for (const name of ['playwright', 'playwright-core', 'three']) copy(resolve(root, 'node_modules', name), 'node_modules/' + name);
copy(resolve(root, 'packages/vrm/src'), 'packages/vrm/src');
copy(resolve(root, 'workers/vrm/src'), 'workers/vrm/src');
copy(resolve(root, 'workers/vrm/dist'), 'workers/vrm/dist');
copy(resolve(root, 'scripts/lib/ssd-runtime.mjs'), 'scripts/lib/ssd-runtime.mjs');
writeFileSync(resolve(stage, 'package.json'), '{"private":true,"type":"module"}\n');
const files = {};
function record(directory, prefix = '') {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const name = prefix + entry.name;
    if (entry.isDirectory()) record(resolve(directory, entry.name), name + '/');
    else if (entry.isFile()) files[name] = createHash('sha256').update(readFileSync(resolve(directory, entry.name))).digest('hex');
    else throw Error('Unexpected runtime symlink: ' + name);
  }
}
record(stage);
writeFileSync(resolve(stage, 'manifest.json'), JSON.stringify({ node: version, nodeArchive: base + archive,
  nodeArchiveSha256: expected, playwright: JSON.parse(readFileSync('node_modules/playwright/package.json')).version,
  files, browser: descriptor, platform: process.platform, arch: process.arch }, null, 2) + '\n');
console.log('VRM runtime staged: ' + stage);
