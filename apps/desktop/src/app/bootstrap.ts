import { locale } from '../shared/i18n/locale.svelte';
import { mount } from 'svelte';
import App from './App.svelte';
import '../shared/theme.css';
import { applyTheme, readTheme } from '../shared/theme';

export function start(target: HTMLElement): void {
  applyTheme(readTheme());
  locale.initialize();
  mount(App, { target });
}
