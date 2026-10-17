import { locale } from '../../shared/i18n/locale.svelte';
import type { VisualFade } from '@sungeki/api-client';
export type SceneFadeInput = { inSeconds: number | undefined; outSeconds: number | undefined };
export function sceneFadeFromForm(input: SceneFadeInput | undefined): VisualFade | undefined {
  if (!input) return undefined;
  const { inSeconds, outSeconds } = input;
  if (inSeconds === undefined || outSeconds === undefined || !Number.isFinite(inSeconds) || !Number.isFinite(outSeconds)
      || inSeconds < 0 || outSeconds < 0 || inSeconds + outSeconds > 604800) {
    throw new Error(locale.text('productionValidationFadeError'));
  }
  return { in_us: Math.round(inSeconds * 1e6), out_us: Math.round(outSeconds * 1e6) };
}
