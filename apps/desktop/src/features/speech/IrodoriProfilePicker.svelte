<script lang="ts">
  import ProfileConfigurationHelp from './ProfileConfigurationHelp.svelte';
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { IrodoriProfiles } from './irodori-profiles.svelte';
  let { profiles, disabled = false }: { profiles: IrodoriProfiles; disabled?: boolean } = $props();
</script>
<section aria-label={locale.text('voiceProfileTitle').replace('{engine}', 'Irodori')}>
  <label>{locale.text('voiceProfileTitle').replace('{engine}', 'Irodori')}
    <select bind:value={profiles.selectedId} disabled={disabled || profiles.busy || !profiles.loaded || !profiles.profiles.length}>
      <option value="">{locale.text('voiceProfileSelect')}</option>
      {#each profiles.profiles as profile (profile.id)}
        <option value={profile.id}>{profile.name}{profiles.profiles.filter(p => p.name === profile.name).length > 1 ? `［${profile.id}］` : ''}</option>
      {/each}
    </select>
  </label>
  <button type="button" disabled={disabled || profiles.busy} onclick={() => profiles.refresh()}>{locale.text('voiceProfileRefresh')}</button>
  {#if profiles.busy}<p role="status">{locale.text('voiceProfileLoading')}</p>{/if}
  <ProfileConfigurationHelp path={profiles.configurationPath} />
  {#if profiles.error && !profiles.configurationPath}<p role="alert">{profiles.error}</p>{/if}
  {#if profiles.loaded && !profiles.profiles.length}<p>{locale.text('voiceProfileEmpty')}</p>{/if}
  <p>{locale.text('voiceProfileHint')}</p>
</section>
<style>
  section { padding:12px; margin-bottom:12px; border:1px solid var(--panel-border); border-radius:6px; background:var(--background); }
  label { display:flex; flex-direction:column; gap:6px; margin-bottom:10px; }
  select { width:100%; }
</style>
