// Produce a self-consistent local macOS bundle, without a distribution identity.
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

if (process.platform !== 'darwin') throw new Error('ローカル.app作成はmacOSで実行してください');
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const run = (command, args) => execFileSync(command, args, { cwd: root, stdio: 'inherit' });
const args = process.argv.slice(2);
if (args.some((arg) => arg !== '--dist')) throw new Error('対応する指定は --dist のみです');
const profile = args.includes('--dist') ? 'dist' : 'release';
const target = process.arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin';
run(process.env.SUNGEKI_BUILD_PYTHON || 'python3', ['-X', 'utf8',
  'scripts/prepare-native-notices.py', '--target', target]);
run(process.execPath, ['scripts/prepare-vrm-bundle.mjs']);
const configuration = JSON.parse(readFileSync(resolve(root, 'apps/desktop/src-tauri/tauri.conf.json'), 'utf8'));
const bundleConfig = JSON.stringify({ bundle: { active: true, resources: {
  ...configuration.bundle.resources, 'resources/native-notices/': 'native-notices/',
  'resources/vrm-runtime/': 'vrm-runtime/',
  '../dist/THIRD_PARTY_FRONTEND.txt': 'THIRD_PARTY_FRONTEND.txt',
} } });
// The standalone bundle command cannot select a custom Cargo profile.
// Keep the build and bundle together so Tauri resolves the dist output directory.
const command = profile === 'dist'
  ? ['build', '--bundles', 'app', '--config', bundleConfig, '--', '--profile', 'dist', '--locked']
  : ['bundle', '--bundles', 'app', '--config', bundleConfig];
run('npm', ['run', 'tauri', '-w', '@sungeki/desktop', '--', ...command]);
const bundle = resolve(root, `target/${profile}/bundle/macos/${configuration.productName}.app`);
const iconName = execFileSync('/usr/libexec/PlistBuddy', [
  '-c', 'Print :CFBundleIconFile', resolve(bundle, 'Contents/Info.plist'),
], { encoding: 'utf8' }).trim();
if (iconName !== 'icon.icns' || !readFileSync(resolve(bundle, 'Contents/Resources', iconName))
  .equals(readFileSync(resolve(root, 'apps/desktop/src-tauri/icons/icon.icns')))) {
  throw new Error('アプリアイコンの登録または同梱内容が一致しません');
}
// Check the packaged artifact, not only the resource configuration.
if (!readFileSync(resolve(bundle, 'Contents/Resources/LICENSE')).equals(readFileSync(resolve(root, 'LICENSE')))) {
  throw new Error('同梱LICENSEが本体のLICENSEと一致しません');
}
if (!readFileSync(resolve(bundle, 'Contents/Resources/THIRD_PARTY_FRONTEND.txt'))
  .equals(readFileSync(resolve(root, 'apps/desktop/dist/THIRD_PARTY_FRONTEND.txt')))) {
  throw new Error('画面側の第三者通知がビルド結果と一致しません');
}
const stagedNotices = resolve(root, 'apps/desktop/src-tauri/resources/native-notices');
const bundledNotices = resolve(bundle, 'Contents/Resources/native-notices');
const noticeManifest = readFileSync(resolve(stagedNotices, 'manifest.json'));
if (!noticeManifest.equals(readFileSync(resolve(bundledNotices, 'manifest.json')))) {
  throw new Error('同梱した第三者通知のmanifestが一致しません');
}
for (const [file, expected] of Object.entries(JSON.parse(noticeManifest.toString()).files)) {
  if (file.startsWith('/') || file.split('/').includes('..')) throw new Error('通知のパスが不正です');
  const actual = createHash('sha256').update(readFileSync(resolve(bundledNotices, file))).digest('hex');
  if (actual !== expected) throw new Error(`同梱した第三者通知が一致しません: ${file}`);
}
const vrmRoot = resolve(bundle, 'Contents/Resources/vrm-runtime');
const vrmManifest = JSON.parse(readFileSync(resolve(vrmRoot, 'manifest.json')));
for (const [file, expected] of Object.entries(vrmManifest.files)) {
  if (file.startsWith('/') || file.split('/').includes('..')) throw Error('Invalid VRM resource path');
  if (createHash('sha256').update(readFileSync(resolve(vrmRoot, file))).digest('hex') !== expected)
    throw Error('VRM resource mismatch: ' + file);
}
// Sign nested code first, then seal the bundle's resources. '-' is ad-hoc only.
for (const name of ['sungeki-host', 'sungeki-mcp']) {
  const executable = resolve(bundle, 'Contents/MacOS', name);
  if (!readFileSync(executable).equals(readFileSync(resolve(root, `target/${profile}/${name}`)))) {
    throw new Error(`同梱プログラムがビルド結果と一致しません: ${name}`);
  }
  run('/usr/bin/codesign', ['--force', '--sign', '-', executable]);
}
run('/usr/bin/codesign', ['--force', '--sign', '-', resolve(bundle, 'Contents/Resources/vrm-runtime/bin/node')]);
vrmManifest.files['bin/node'] = createHash('sha256').update(readFileSync(resolve(vrmRoot, 'bin/node'))).digest('hex');
writeFileSync(resolve(vrmRoot, 'manifest.json'), JSON.stringify(vrmManifest, null, 2) + '\n');
run('/usr/bin/codesign', ['--force', '--sign', '-', bundle]);
run('/usr/bin/codesign', ['--verify', '--deep', '--strict', bundle]);
run(process.execPath, ['scripts/check-mcp.mjs', resolve(bundle, 'Contents/MacOS/sungeki-mcp')]);
console.log('ローカル.appの署名整合性を確認しました（配布署名・公証ではありません）');
