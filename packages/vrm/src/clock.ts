export const MAX_VRM_TIME_US = 7_200_000_000;
/** Exact integer admission and fixed 60 Hz steps, plus one residual step. */
export function vrmClock(timeUs: number) {
  if (!Number.isSafeInteger(timeUs) || timeUs < 0 || timeUs > MAX_VRM_TIME_US) {
    throw new Error('VRMソース時刻は0〜2時間の整数マイクロ秒で指定してください');
  }
  const scaled = timeUs * 60;
  return { tick: Math.floor(scaled / 1_000_000), residual: (scaled % 1_000_000) / 60_000_000,
    seconds: timeUs / 1_000_000 };
}
