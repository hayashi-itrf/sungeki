import { ssdRuntime } from '../../../scripts/lib/ssd-runtime.mjs';
import { mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';

export async function runtimePaths(configuredRoot?: string) {
  const actual = await ssdRuntime(configuredRoot);
  const browsers = resolve(actual, 'playwright');
  const work = resolve(actual, 'vrm-work');
  const temporary = resolve(actual, 'tmp');
  for (const directory of [browsers, work, temporary]) await mkdir(directory, { recursive: true });
  return { root: actual, browsers, work, temporary };
}

export function browserEnvironment(paths: Awaited<ReturnType<typeof runtimePaths>>) {
  return { ...process.env, PLAYWRIGHT_BROWSERS_PATH: paths.browsers,
    TMPDIR: paths.temporary, TMP: paths.temporary, TEMP: paths.temporary };
}
