import { locale } from '../../shared/i18n/locale.svelte';
import type { ReviewFrameOptions } from '@sungeki/api-client';
export function parseSampling(text: string, limit: number | undefined, margin: number | undefined): ReviewFrameOptions {
  if (limit === undefined || !Number.isInteger(limit) || limit < 1 || limit > 120) throw new Error(locale.text('reportFrameLimitError'));
  if (margin === undefined || !Number.isInteger(margin) || margin < 0 || margin > 25) throw new Error(locale.text('reportMarginError'));
  const values = text.trim() ? text.trim().split(/[,、\s]+/) : [];
  if (values.length > 120) throw new Error(locale.text('reportTimesLimitError'));
  const requested_times_us = values.map(value => {
    if (!/^\d+(\.\d{1,6})?$/.test(value)) throw new Error(locale.text('reportTimeError'));
    const [whole, fraction = ''] = value.split('.');
    const us = Number(whole) * 1_000_000 + Number(fraction.padEnd(6, '0'));
    if (!Number.isSafeInteger(us)) throw new Error(locale.text('reportTimeRangeError'));
    return us;
  });
  return { max_frames: limit, requested_times_us, text_safe_margin_percent: margin };
}
