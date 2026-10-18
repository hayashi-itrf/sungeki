import { readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';

const root = new URL('../', import.meta.url);
const manifest = JSON.parse(readFileSync(new URL('package.json', root), 'utf8'));
const nodePin = readFileSync(new URL('.node-version', root), 'utf8').trim();
const npmPin = manifest.packageManager.replace(/^npm@/, '');
let npmVersion;
try {
  npmVersion = execFileSync('npm', ['--version'], { encoding: 'utf8', timeout: 10000, stdio: ['ignore', 'pipe', 'pipe'] }).trim();
} catch {
  console.error('npmを起動できません。Nodeの配布物に含まれるbinフォルダーをPATHへ追加してください。導入手順: docs/INSTALLATION.md');
  process.exit(1);
}
const problems = [];
if (manifest.engines.node !== nodePin) problems.push('.node-versionとengines.nodeが不一致');
if (manifest.engines.npm !== npmPin) problems.push('packageManagerとengines.npmが不一致');
if (process.versions.node !== nodePin) problems.push(`Node: ${process.versions.node}（必要: ${nodePin}）`);
if (npmVersion !== npmPin) problems.push(`npm: ${npmVersion}（必要: ${npmPin}）`);
if (problems.length) {
  console.error(`実行環境がプロジェクトの固定版と一致しません。PATHを確認してください。導入手順: docs/INSTALLATION.md\n${problems.join('\n')}`);
  process.exitCode = 1;
} else {
  console.log(`固定環境確認: Node ${nodePin} / npm ${npmPin} (${process.execPath})`);
}
