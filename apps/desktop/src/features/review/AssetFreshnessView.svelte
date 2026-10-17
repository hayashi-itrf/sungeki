<script lang="ts">
  import { freshnessDescription } from './diagnostic-text';
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { AssetFreshness } from '@sungeki/api-client';
  let { freshness, watchable = true }: { freshness: AssetFreshness; watchable?: boolean } = $props();
  const labels = $derived({ unchecked: locale.text('freshUnchecked'), matching: locale.text('freshMatching'), changed: locale.text('freshChanged'), unavailable: locale.text('freshUnavailable') });
</script>
<div class="freshness" role="status">
  <strong>{locale.text('freshTitle')}{labels[freshness.status]}</strong>
  {#if freshness.checking}<span>{locale.text('freshChecking')}</span>{/if}
  {#if freshness.message}<p>{freshnessDescription(freshness.message)}</p>{/if}
  {#if freshness.checked_at_ms !== null}<p>{locale.text('freshTime')}{new Date(freshness.checked_at_ms).toLocaleString(locale.current === 'ja' ? 'ja-JP' : 'en-US')}</p>{/if}
  <p>{watchable ? locale.text('freshWatch') : locale.text('freshMissing')}</p>
</div>
<style>
  .freshness { margin:12px 0; padding:10px; border:1px solid var(--panel-border); border-radius:4px; background:var(--background); }
  span { margin-left:8px; color:var(--muted); }
  p { margin:6px 0 0; font-size:12px; }
</style>
