<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { buildInfo } from '../../shared/build-info';
  const builtAt = $derived(new Date(buildInfo.builtAt).toLocaleString(locale.current === 'ja' ? 'ja-JP' : 'en-US', { timeZoneName: 'short' }));
</script>
<section aria-label={locale.current === 'ja' ? 'このアプリについて' : 'About this app'}>
  <strong>Sungeki</strong>
  <dl>
    <dt>{locale.current === 'ja' ? 'バージョン' : 'Version'}</dt><dd>{buildInfo.version}</dd>
    <dt>{locale.current === 'ja' ? 'ビルド日時' : 'Built at'}</dt><dd>{builtAt}</dd>
    <dt>{locale.current === 'ja' ? '識別番号' : 'Build ID'}</dt><dd>{buildInfo.id}</dd>
  </dl>
</section>
<style>
  section { padding:12px; border:1px solid var(--border); border-radius:6px; margin-bottom:12px; }
  dl { display:grid; grid-template-columns:auto 1fr; gap:4px 12px; margin:8px 0 0; font-size:12px; }
  dd { margin:0; overflow-wrap:anywhere; user-select:text; }
</style>
