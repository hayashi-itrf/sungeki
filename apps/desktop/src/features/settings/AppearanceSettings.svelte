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

<section>
  <h2>{locale.text('appearance')}</h2>
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
</section>

<style>
  .appearance { display:grid; grid-template-columns:max-content minmax(160px,300px); gap:14px; align-items:center; }
  .appearance p { grid-column:1 / -1; margin:4px 0; }
  @media(max-width:600px) { .appearance { grid-template-columns:1fr; } }
</style>
