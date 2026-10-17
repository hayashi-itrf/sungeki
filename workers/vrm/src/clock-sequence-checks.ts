import type { Page } from 'playwright';
import type {} from './browser.ts';

export async function inspectClockSequence(page: Page) {
  // Dense rational-frame requests exercise fractional poses between fixed ticks.
  return page.evaluate(async () => {
    const first = await window.vvmVrmProbe.frameAt(0, true);
    const expected = new Map<number, string>();
    let last = first;
    for (let frame = 1; frame <= 90; frame++) {
      const us = Math.floor(frame * 1001000000 / 30000);
      last = await window.vvmVrmProbe.frameAt(us);
      if ([1, 29, 30, 59, 90].includes(frame)) expected.set(us, last.png);
    }
    if (last.modelLoads !== first.modelLoads) throw new Error('前進中にVRMを再読込しました');
    const steps = last.simulationSteps - first.simulationSteps;
    if (steps !== 180) throw new Error(`物理計算回数が時刻に一致しません: ${steps}`);
    for (const [us, png] of expected) {
      const fresh = await window.vvmVrmProbe.frameAt(us, true);
      if (fresh.png !== png) throw new Error(`連続描画とfreshが一致しません: ${us}`);
    }
    return { status: 'passed', frames: 90, reloadsDuringForward: 0, simulationSteps: steps, freshMatches: expected.size };
  });
}
