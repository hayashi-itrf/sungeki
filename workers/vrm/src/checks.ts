import { inspectClockSequence } from './clock-sequence-checks.ts';
import { inspectTimes } from './time-checks.ts';
import { inspectExpressions } from './expression-checks.ts';
import { createHash } from 'node:crypto';
import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { Page } from 'playwright';
import type {} from './browser.ts';

export async function inspectFrames(page: Page, directory: string, signal: AbortSignal, cases: unknown[]) {
  for (const [name, width, height] of [['landscape', 640, 360], ['portrait', 360, 640]] as const) {
    signal.throwIfAborted();
    const info = await page.evaluate(async ({ width, height }) => window.vvmVrmProbe.load(width, height), { width, height });
    const frames: unknown[] = [];
    const result = { name, width, height, info, frames, seekExactMatch: false, expressions: {} as unknown, fractionalExpressions: {} as unknown, times: {} as unknown, clockSequence: {} as unknown };
    cases.push(result);
    if (!info.webglVersion.includes('WebGL 2.0')) throw new Error('WebGL2以外の描画です');
    if (/swiftshader|software|llvmpipe|unavailable/i.test(info.renderer)) throw new Error('ソフトウェア描画では実GPU検証を合格にしません');
    const hashes = new Map<number, string>();
    const sampleFrames = [0, 30, 60, Math.ceil(info.duration * 30) + 15];
    const samples = [...sampleFrames.map((frame) => ({ frame, fresh: false })),
      ...[60, 30, 0].map((frame) => ({ frame, fresh: true }))];
    for (const sample of samples) {
      signal.throwIfAborted();
      const frame = await page.evaluate(async ({ frame, fresh }) => window.vvmVrmProbe.frame(frame, fresh), sample);
      const bytes = Buffer.from(frame.png.split(',')[1], 'base64');
      const hash = createHash('sha256').update(bytes).digest('hex');
      if (frame.pixels.visible < 200 || frame.pixels.transparent < 200 || frame.calls === 0 || frame.triangles === 0) {
        throw new Error('有効な透過モデル画像ではありません');
      }
      if (frame.pixels.edge !== 0) throw new Error('モデルがフレーム端に接触しています');
      if (sample.fresh && hashes.get(frame.frame) !== hash) throw new Error(`シーク結果が一致しません: ${name}/${frame.frame}`);
      if (!sample.fresh) hashes.set(frame.frame, hash);
      const filename = `${name}-${String(frame.frame).padStart(4, '0')}-${sample.fresh ? 'seek' : 'forward'}.png`;
      await writeFile(join(directory, filename), bytes, { flag: 'wx' });
      if (name === 'landscape' && sample.frame === 0 && !sample.fresh) {
        process.stdout.write(JSON.stringify({ type: 'stage', stage: 'frame_rendered' }) + '\n');
      }
      frames.push({ filename, frame: frame.frame, seconds: frame.seconds, sha256: hash, pixels: frame.pixels });
    }
    if (new Set(hashes.values()).size < 3) throw new Error('VRMAの動きが画像へ反映されていません');
    result.seekExactMatch = true;
    result.expressions = await inspectExpressions(page, directory, name, info.expressions);
    result.fractionalExpressions = await inspectExpressions(page, directory, `${name}-fractional`, info.expressions, 1234567);
    result.times = await inspectTimes(page, directory, name);
    result.clockSequence = await inspectClockSequence(page);
  }
  await page.evaluate(() => window.vvmVrmProbe.dispose());
  return cases;
}
