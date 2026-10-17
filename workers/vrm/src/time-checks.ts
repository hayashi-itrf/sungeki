import { createHash } from 'node:crypto';
import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { Page } from 'playwright';
import type {} from './browser.ts';

export async function inspectTimes(page: Page, directory: string, name: string) {
  const hashes = new Map<number, string>();
  const times = [0, 33367, 41667, 1234567];
  const results = [];
  for (const [pass, samples] of [['forward', times], ['seek', [...times].reverse()]] as const) {
    for (const timeUs of samples) {
      const result = await page.evaluate(async ({ timeUs, fresh }) =>
        window.vvmVrmProbe.frameAt(timeUs, fresh), { timeUs, fresh: pass === 'seek' });
      if (result.timeUs !== timeUs || result.seconds !== timeUs / 1e6) throw new Error('ソース時刻が一致しません');
      const bytes = Buffer.from(result.png.split(',')[1], 'base64');
      const hash = createHash('sha256').update(bytes).digest('hex');
      if (pass === 'seek' && hashes.get(timeUs) !== hash) throw new Error(`任意時刻のシークが一致しません: ${timeUs}`);
      if (pass === 'forward') hashes.set(timeUs, hash);
      const filename = `${name}-time-${timeUs}-${pass}.png`;
      await writeFile(join(directory, filename), bytes, { flag: 'wx' });
      results.push({ timeUs, pass, sha256: hash, filename });
    }
  }
  const exact = await page.evaluate(async () => {
    await window.vvmVrmProbe.frameAt(1234567);
    const before = await window.vvmVrmProbe.frameAt(1000000);
    const after = await window.vvmVrmProbe.frame(30, true);
    return before.png === after.png;
  });
  if (!exact) throw new Error('端数時刻の後に固定時刻の状態が変わりました');
  return { status: 'passed', samples: results, exactTickMatch: exact };
}
