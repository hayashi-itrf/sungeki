<script lang="ts">
  import { readTheme, saveTheme } from '../../shared/theme';
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { MessageKey } from '../../shared/i18n/messages';
  let theme = $state(readTheme());
  let message = $state<MessageKey>('preferenceHint');
  function changeTheme(event: Event) {
    const value = (event.currentTarget as HTMLSelectElement).value;
    theme = value === 'dark' ? 'dark' : 'light';
    message = saveTheme(theme) ? 'preferenceSaved' : 'preferenceFailed';
  }
  function changeLanguage(event: Event) {
    const value = (event.currentTarget as HTMLSelectElement).value;
    message = locale.set(value === 'en' ? 'en' : 'ja') ? 'preferenceSaved' : 'preferenceFailed';
  }
</script>

<details class="settings">
  <summary>{locale.text('appearance')}</summary>
  <div class="appearance">
    <label for="appearance-theme">{locale.text('theme')}</label>
    <select id="appearance-theme" value={theme} onchange={changeTheme}>
      <option value="light">{locale.text('light')}</option>
      <option value="dark">{locale.text('dark')}</option>
    </select>
    <label for="appearance-language">{locale.text('language')}</label>
    <select id="appearance-language" value={locale.current} onchange={changeLanguage}>
      <option value="ja">日本語</option>
      <option value="en">English</option>
    </select>
    <p>{locale.text('languageScope')}</p>
    <p role="status">{locale.text(message)}</p>
  </div>
</details>
