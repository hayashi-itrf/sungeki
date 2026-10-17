import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import { dirname, resolve } from 'node:path';
import { browserEnvironment, runtimePaths } from './runtime.ts';

const paths = await runtimePaths();
const require = createRequire(import.meta.url);
const cli = resolve(dirname(require.resolve('playwright/package.json')), 'cli.js');
// run.ts uses channel: 'chromium'; the separate headless-shell binary is unused.
const child = spawn(process.execPath, [cli, 'install', 'chromium', '--no-shell'], {
  env: browserEnvironment(paths), stdio: 'inherit',
});
child.on('error', (error) => { console.error(error); process.exitCode = 1; });
child.on('exit', (code) => { process.exitCode = code ?? 1; });
