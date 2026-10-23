import { ssdRuntime } from '../../../scripts/lib/ssd-runtime.mjs';
import { mkdir } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';

export async function runtimePaths(configuredRoot?: string) {
  const actual = await ssdRuntime(configuredRoot);
  const packaged = fileURLToPath(new URL('../../../browser', import.meta.url));
  const browsers = existsSync(packaged) ? packaged : resolve(actual, 'playwright');
  const executable = existsSync(packaged)
    ? resolve(packaged, `chrome-headless-shell-mac-${process.arch === 'arm64' ? 'arm64' : 'x64'}`, 'chrome-headless-shell') : undefined;
  const work = resolve(actual, 'vrm-work');
  const temporary = resolve(actual, 'tmp');
  for (const directory of [work, temporary]) await mkdir(directory, { recursive: true });
  return { root: actual, browsers, executable, work, temporary };
}

export function browserEnvironment(paths: Awaited<ReturnType<typeof runtimePaths>>) {
  return { ...process.env, PLAYWRIGHT_BROWSERS_PATH: paths.browsers,
    TMPDIR: paths.temporary, TMP: paths.temporary, TEMP: paths.temporary };
}
