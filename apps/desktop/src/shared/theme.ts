export type Theme = 'light' | 'dark';
const storageKey = 'sungeki.ui.theme';

export function readTheme(): Theme {
  try {
    return localStorage.getItem(storageKey) === 'dark' ? 'dark' : 'light';
  } catch {
    return 'light';
  }
}

export function applyTheme(theme: Theme): void {
  document.documentElement.dataset.theme = theme;
}

export function saveTheme(theme: Theme): boolean {
  applyTheme(theme);
  try {
    localStorage.setItem(storageKey, theme);
    return true;
  } catch {
    return false;
  }
}
