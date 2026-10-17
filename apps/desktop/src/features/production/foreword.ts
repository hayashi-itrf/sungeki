import { locale } from '../../shared/i18n/locale.svelte';
import type { BuildForeword } from '@sungeki/api-client';
export type ForewordInput = { enabled: boolean; text: string; seconds: number | undefined };
export function forewordFromForm(input?: ForewordInput, portrait = false): BuildForeword | null {
  if (!input?.enabled) return null;
  const text = input.text.replace(/\r\n?/g, '\n');
  if (!text.trim() || text.includes('\0') || new TextEncoder().encode(text).length > 4096 || text.split('\n').length > 4) {
    throw new Error(locale.text('productionValidationForewordText'));
  }
  const seconds = input.seconds;
  if (seconds === undefined || !Number.isFinite(seconds) || seconds < 1.6 || seconds > 60) throw new Error(locale.text('productionValidationForewordTime'));
  return { preset_id: portrait ? 'foreword-portrait-v1' : 'foreword-wide-v1', text, duration_us: Math.round(seconds * 1_000_000) };
}
