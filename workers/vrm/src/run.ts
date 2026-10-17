import { mkdir, rename, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { randomUUID } from 'node:crypto';
import type { BrowserContext, Page } from 'playwright';
import { runtimePaths, browserEnvironment } from './runtime.ts';
import { readGlb } from './inputs.ts';
import { serveAssets } from './server.ts';
import { inspectFrames } from './checks.ts';
import { profileProcesses, type OwnedProcess } from './processes.ts';

export async function runProbe(modelPath: string, motionPath: string, signal: AbortSignal) {
  return runWorker(modelPath, motionPath, signal, async (page, staging, report) => {
    report.fps = 30;
    report.cases = [];
    await inspectFrames(page, staging, signal, report.cases as unknown[]);
  });
}
export async function runWorker(modelPath: string, motionPath: string, signal: AbortSignal,
  task: (page: Page, staging: string, report: Record<string, unknown>) => Promise<void>,
  expected?: { model: { sha256: string; byte_length: number }; motion: { sha256: string; byte_length: number } },
  runtimeRoot?: string) {
  const paths = await runtimePaths(runtimeRoot);
  process.env.PLAYWRIGHT_BROWSERS_PATH = paths.browsers;
  const { chromium } = await import('playwright');
  const id = `${expected ? "render" : "probe"}-${randomUUID()}`;
  const staging = join(paths.work, `${id}.partial`);
  await mkdir(staging);
  const profile = join(staging, 'browser-profile');
  const temporary = join(staging, 'tmp');
  await mkdir(temporary);
  let owned: OwnedProcess[] = [];
  let context: BrowserContext | undefined;
  let server: Awaited<ReturnType<typeof serveAssets>> | undefined;
  const warnings: string[] = [];
  const errors: string[] = [];
  const report: Record<string, unknown> = { id, createdAt: new Date().toISOString(), status: 'running', simulationHz: 60 };
  let failure: unknown;
  const cancel = () => { void context?.close().catch(() => {}); };
  signal.addEventListener('abort', cancel, { once: true });
  try {
    signal.throwIfAborted();
    const model = await readGlb(modelPath, 'model');
    const motion = await readGlb(motionPath, 'motion');
    report.inputs = { model: model.manifest, motion: motion.manifest };
    if (expected) {
      for (const [key, input] of [['model', model], ['motion', motion]] as const) {
        if (input.manifest.sha256 !== expected[key].sha256 || input.manifest.bytes !== expected[key].byte_length) {
          throw new Error(`VRM原本が登録内容から変わっています: ${key}`);
        }
      }
    }
    server = await serveAssets(model.bytes, motion.bytes);
    context = await chromium.launchPersistentContext(profile, {
      channel: 'chromium', headless: true, chromiumSandbox: true, timeout: 30000,
      args: ['--use-angle=metal'], ignoreDefaultArgs: ['--enable-unsafe-swiftshader'],
      viewport: { width: 640, height: 640 }, deviceScaleFactor: 1,
      env: browserEnvironment({ ...paths, temporary }),
    });
    signal.throwIfAborted();
    report.browser = context.browser()?.version();
    owned = await profileProcesses(profile);
    report.processes = owned;
    process.stdout.write(JSON.stringify({ type: 'stage', stage: 'browser_started', id }) + '\n');
    await context.route('**/*', (route) => server!.allowed.has(route.request().url()) ? route.continue() : route.abort());
    const page = context.pages()[0] ?? await context.newPage();
    page.setDefaultTimeout(30000);
    page.on('pageerror', (error) => errors.push(error.message));
    page.on('console', (message) => {
      if (message.type() === 'warning') warnings.push(message.text());
      if (message.type() === 'error') errors.push(message.text());
    });
    await page.goto(server.url, { waitUntil: 'load' });
    await page.waitForFunction(() => Boolean(window.vvmVrmProbe));
    await task(page, staging, report);
    if (errors.length > 0) throw new Error(`描画エラー: ${errors.join('; ')}`);
    report.status = 'passed';
  } catch (error) {
    failure = error;
    report.status = signal.aborted ? 'cancelled' : 'failed';
    report.error = String(error);
  } finally {
    signal.removeEventListener('abort', cancel);
    owned = [...owned, ...await profileProcesses(profile)];
    try { await context?.close(); } catch (error) { report.closeError = String(error); failure ??= error; }
    try { await server?.close(); } catch (error) { report.serverCloseError = String(error); failure ??= error; }
    const remaining = await profileProcesses(profile, owned);
    report.remainingProcesses = remaining;
    report.cleanupPassed = remaining.length === 0 && !report.closeError && !report.serverCloseError;
    if (!report.cleanupPassed) { failure ??= new Error('ワーカーの終了確認に失敗しました'); report.status = 'failed'; }
    else {
      await rm(profile, { recursive: true, force: true });
      await rm(temporary, { recursive: true, force: true });
    }
    report.warnings = warnings;
    report.pageErrors = errors;
    await writeFile(join(staging, 'report.json'), JSON.stringify(report, null, 2) + '\n');
  }
  const destination = join(paths.work, failure ? `${id}.${report.status}` : id);
  await rename(staging, destination);
  process.stdout.write(JSON.stringify({ type: 'result', status: report.status, directory: destination }) + '\n');
  if (failure) throw failure;
  return destination;
}
