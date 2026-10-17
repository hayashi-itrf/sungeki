import { english, japanese, type MessageKey } from './messages';
export type Locale = 'ja' | 'en';
const key = 'sungeki.ui.locale';
export function readLocale(): Locale {
  try { return localStorage.getItem(key) === 'en' ? 'en' : 'ja'; }
  catch { return 'ja'; }
}
export class LocaleSettings {
  current = $state<Locale>('ja');
  initialize() { this.current = readLocale(); this.apply(); }
  private apply() { if (typeof document !== 'undefined') document.documentElement.lang = this.current; }
  set(locale: Locale): boolean {
    this.current = locale; this.apply();
    try { localStorage.setItem(key, locale); return true; }
    catch { return false; }
  }
  text(key: MessageKey): string { return (this.current === 'en' ? english : japanese)[key]; }
}
export const locale = new LocaleSettings();
