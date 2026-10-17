import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { runWorker } from './run.ts';
import { readRenderRequest } from './render-request.ts';

export async function renderRequest(path: string, signal: AbortSignal, runtimeRoot: string) {
  const request = await readRenderRequest(path);
  return runWorker(request.model.path, request.motion.path, signal, async (page, staging, report) => {
    const started = performance.now();
    const info = await page.evaluate(async ({ width, height }) => window.vvmVrmProbe.load(width, height), { width: request.width, height: request.height });
    if (!info.webglVersion.includes('WebGL 2.0') || /swiftshader|software|llvmpipe|unavailable/i.test(info.renderer)) {
      throw new Error('VRM正式描画に必要な実GPU/WebGL2を確認できません');
    }
    report.requestVersion = request.version; report.width = request.width; report.height = request.height; report.info = info;
    report.frames = []; let totalBytes = 0;
    for (const [index, frame] of request.frames.entries()) {
      signal.throwIfAborted();
      const result = await page.evaluate(async frame => window.vvmVrmProbe.frameAt(frame.time_us, false, frame.expressions, frame.pose), frame);
      if (result.timeUs !== frame.time_us || !result.png.startsWith('data:image/png;base64,')) throw new Error('描画応答が要求と一致しません');
      if (result.pixels.visible === 0 || result.pixels.edge !== 0) throw new Error(`VRMが画面外または端に接触しています（時刻 ${frame.time_us}µs）。`);
      const png = Buffer.from(result.png.split(',')[1], 'base64');
      totalBytes += png.length;
      if (totalBytes > 1024*1024*1024) throw new Error('VRM画像の合計が1GiBを超えています');
      const filename = `frame-${String(index).padStart(6, '0')}.png`;
      await writeFile(join(staging, filename), png, { flag: 'wx' });
      (report.frames as unknown[]).push({ index, time_us: frame.time_us, expressions: frame.expressions, pose: frame.pose ?? {}, filename, bytes: png.length,
        modelLoads: result.modelLoads, simulationSteps: result.simulationSteps,
        sha256: createHash('sha256').update(png).digest('hex'), pixels: result.pixels });
    }
    signal.throwIfAborted();
    await page.evaluate(() => window.vvmVrmProbe.dispose());
    report.renderDurationMs = performance.now() - started;
  }, request, runtimeRoot);
}
