import { createHash } from 'node:crypto';
import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { ExpressionWeights } from '@sungeki/vrm';
import type { Page } from 'playwright';
import type {} from './browser.ts';

export async function inspectExpressions(page: Page, directory: string, name: string, available: string[], timeUs = 1000000) {
  if (!available.includes('aa')) return { status: 'not_run', reason: 'モデルにaa表情がありません' };
  const hashes: string[] = [];
  // Same timestamp: override, remove, reapply, fresh seek. No motion delta can hide contamination.
  const samples: [string, ExpressionWeights, boolean][] = [
    ['base', {}, true], ['open', { aa: 1 }, false], ['restored', {}, false],
    ['repeat', { aa: 1 }, false], ['seek', { aa: 1 }, true],
  ];
  for (const [label, weights, fresh] of samples) {
    const result = await page.evaluate(async ({ weights, fresh, timeUs }) =>
      window.vvmVrmProbe.frameAt(timeUs, fresh, weights), { weights, fresh, timeUs });
    const bytes = Buffer.from(result.png.split(',')[1], 'base64');
    hashes.push(createHash('sha256').update(bytes).digest('hex'));
    await writeFile(join(directory, `${name}-expression-${label}.png`), bytes, { flag: 'wx' });
  }
  if (hashes[0] === hashes[1]) throw new Error('aa表情が画素へ反映されていません');
  if (hashes[0] !== hashes[2]) throw new Error('前フレームの表情が残っています');
  if (hashes[1] !== hashes[3] || hashes[1] !== hashes[4]) throw new Error('表情指定の再表示/シークが一致しません');
  return { status: 'passed', hashes, expression: 'aa' };
}
