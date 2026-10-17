import { execFileSync } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { ssdRuntime } from './lib/ssd-runtime.mjs';
import { desktopEnvironment } from './lib/desktop-environment.mjs';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const env = await desktopEnvironment(root, await ssdRuntime(), process.env, process.execPath, message => console.warn(message));
// Build shared GPU code before the GUI/host uses it; no model or engine is started.
execFileSync('npm', ['run', 'build:vrm'], { cwd: root, env, stdio: 'inherit' });
execFileSync('npm', ['run', 'tauri', '-w', '@sungeki/desktop', '--', 'dev', '--release'],
  { cwd: root, env, stdio: 'inherit' });
