import { execFileSync } from 'node:child_process';
import { mkdir, readFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { runtimeLocation, ssdRuntime } from './lib/ssd-runtime.mjs';

async function main() {
  const mode = process.argv[2];
  if (!['setup', 'lock', 'probe'].includes(mode)) throw new Error('setup / lock / probe を指定してください');
  runtimeLocation(process.env.SUNGEKI_RUNTIME_ROOT);
  let version;
  try {
    version = execFileSync('uv', ['--version'], { encoding: 'utf8', timeout: 10000 }).trim().split(/\s+/)[1];
  } catch (error) {
    if (error.code === 'ENOENT') throw new Error('uv 0.10.6が見つかりません。uvを導入し、このターミナルで uv --version が実行できることを確認してください。');
    throw new Error(`uvの起動確認に失敗しました。uv --version を確認してください: ${error.message}`);
  }
  if (version !== '0.10.6') throw new Error(`uv 0.10.6が必要です（検出: ${version}）。指定版へ切り替えてください。`);
  const project = resolve(dirname(fileURLToPath(import.meta.url)), '../workers/image');
  const runtime = join(await ssdRuntime(), 'image');
  const temporary = join(runtime, 'tmp');
  await mkdir(temporary, { recursive: true });
  const env = { ...process.env, UV_CACHE_DIR: join(runtime, 'uv-cache'),
    UV_PYTHON_INSTALL_DIR: join(runtime, 'python'), UV_PROJECT_ENVIRONMENT: join(runtime, 'venv'),
    UV_PYTHON_BIN_DIR: join(runtime, 'bin'), UV_MANAGED_PYTHON: 'true',
    UV_LINK_MODE: 'copy', TMPDIR: temporary, PYTHONDONTWRITEBYTECODE: '1' };
  const python = (await readFile(join(project, '.python-version'), 'utf8')).trim();
  const run = (args) => execFileSync('uv', args, { cwd: project, env, stdio: 'inherit' });
  if (mode === 'setup') {
    run(['python', 'install', python, '--no-bin']);
    run(['sync', '--locked', '--no-dev']);
  } else if (mode === 'lock') {
    run(['python', 'install', python, '--no-bin']);
    run(['lock']);
  } else if (mode === 'probe') {
    run(['run', '--locked', '--offline', '--no-dev', 'python', 'probe.py', '--output-root', join(runtime, 'probes')]);
  }
}

try { await main(); }
catch (error) {
  console.error(`画像処理環境の処理を停止しました: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
}
