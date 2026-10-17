<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { onMount, onDestroy, untrack } from 'svelte';
  import { Sbv2Profiles } from '../speech/profiles.svelte';
  import { QwenProfiles } from '../speech/qwen-profiles.svelte';
  import { IrodoriProfiles } from '../speech/irodori-profiles.svelte';
  import { hostApi } from '../../shared/host-api';
  import type { BuildProfile, ManagedVoice } from './voice-settings';
  let { engine, person, disabled = false }: { engine: ManagedVoice; person: { profile?: BuildProfile }; disabled?: boolean } = $props();
  // The caller keys this component by character and engine.
  const profiles = untrack(() => engine === 'irodori' ? new IrodoriProfiles(hostApi)
    : engine === 'qwen' ? new QwenProfiles(hostApi) : new Sbv2Profiles(hostApi));
  profiles.selectedId = untrack(() => person.profile?.engine === engine ? person.profile.id : '');
  const label = $derived(engine === 'irodori' ? 'Irodori' : engine === 'qwen' ? 'Qwen' : 'Style-Bert-VITS2');
  onMount(() => { void profiles.refresh(); });
  onDestroy(() => profiles.dispose());
  $effect(() => { person.profile = { engine, id: profiles.selectedId }; });
</script>
<section aria-label={`${label}${locale.text('productionProfile')}`}>
  <label>{label}{locale.text('productionProfile')}
    <select required bind:value={profiles.selectedId} disabled={disabled || profiles.busy || !profiles.loaded}>
      <option value="">{locale.text('productionProfileChoose')}</option>
      {#each profiles.profiles as p (p.id)}<option value={p.id}>{p.name}（{p.id}）</option>{/each}
    </select>
  </label>
  <button type="button" disabled={disabled || profiles.busy} onclick={() => profiles.refresh()}>{locale.text('productionProfileRefresh')}</button>
  {#if profiles.busy}<p role="status">{locale.text('productionProfileChecking')}</p>{/if}
  {#if profiles.error}<p role="alert">{profiles.error}</p>{/if}
  {#if profiles.loaded && !profiles.profiles.length}<p>{locale.text('productionProfileEmpty')}</p>{/if}
  <small>{locale.text('productionProfileHint')}</small>
</section>
<style>
  section { padding:12px; border:1px solid var(--panel-border); border-radius:6px; background:var(--background); }
  label { display:grid; gap:6px; margin-bottom:8px; } select { width:100%; } small { display:block; margin-top:8px; color:var(--muted); }
</style>
