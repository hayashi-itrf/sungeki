import { afterEach, expect, test, vi } from 'vitest';
import { LocaleSettings, readLocale } from './locale.svelte';
import { english, japanese } from './messages';
afterEach(() => vi.unstubAllGlobals());
function storage(value: string | null = null) {
  const values = new Map<string, string>([['sungeki.ui.theme', 'light']]);
  if (value !== null) values.set('sungeki.ui.locale', value);
  vi.stubGlobal('localStorage', { getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => values.set(key, value) });
  vi.stubGlobal('document', { documentElement: { lang: '' } });
  return values;
}
test('Japanese is the default; an unsupported stored locale does not opt into English', () => {
  storage(); expect(readLocale()).toBe('ja');
  storage('fr'); expect(readLocale()).toBe('ja');
});
test('language persists across initialization without changing the light theme', () => {
  const values = storage();
  const initial = new LocaleSettings(); initial.initialize();
  expect(document.documentElement.lang).toBe('ja');
  expect(initial.set('en')).toBe(true); expect(initial.text('save')).toBe('Save');
  const reopened = new LocaleSettings(); reopened.initialize();
  expect(reopened.current).toBe('en'); expect(document.documentElement.lang).toBe('en');
  expect(values.get('sungeki.ui.theme')).toBe('light');
  reopened.set('ja'); expect(reopened.text('save')).toBe('保存');
});
test('storage denial retains the in-session language and reports that it was not saved', () => {
  storage(); vi.stubGlobal('localStorage', { getItem: () => { throw Error('denied'); }, setItem: () => { throw Error('denied'); } });
  const state = new LocaleSettings(); state.initialize(); expect(state.current).toBe('ja');
  expect(state.set('en')).toBe(false); expect(state.text('close')).toBe('Close');
  expect(document.documentElement.lang).toBe('en');
});
test('both dictionaries cover every registered control with nonempty text', () => {
  expect(Object.keys(japanese).sort()).toEqual(Object.keys(english).sort());
  expect([...Object.values(japanese), ...Object.values(english)].every(s => s.trim().length > 0)).toBe(true);
});
