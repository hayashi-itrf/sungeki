import { readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';

const root = new URL('../', import.meta.url);
const manifest = JSON.parse(readFileSync(new URL('package.json', root), 'utf8'));
const nodePin = readFileSync(new URL('.node-version', root), 'utf8').trim();
const npmPin = manifest.packageManager.replace(/^npm@/, '');
const npmVersion = execFileSync('npm', ['--version'], { encoding: 'utf8' }).trim();
const problems = [];
if (manifest.engines.node !== nodePin) problems.push('.node-versionとengines.nodeが不一致');
if (manifest.engines.npm !== npmPin) problems.push('packageManagerとengines.npmが不一致');
if (process.versions.node !== nodePin) problems.push(`Node: ${process.versions.node}（必要: ${nodePin}）`);
if (npmVersion !== npmPin) problems.push(`npm: ${npmVersion}（必要: ${npmPin}）`);
if (problems.length) {
  console.error(`実行環境がプロジェクトの固定版と一致しません。PATHを確認してください。\n${problems.join('\n')}`);
  process.exitCode = 1;
} else {
  console.log(`固定環境確認: Node ${nodePin} / npm ${npmPin} (${process.execPath})`);
}
