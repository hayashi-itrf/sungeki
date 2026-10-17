<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import { qwenLanguageLabel } from './qwen-language';
  import { onDestroy, onMount } from 'svelte';
  import type { EditorController } from '../editor/controller.svelte';
  import { hostApi } from '../../shared/host-api';
  import { Sbv2Profiles } from './profiles.svelte';
  import Sbv2ProfilePicker from './Sbv2ProfilePicker.svelte';
  import { Sbv2CharacterDraft } from './sbv2-character.svelte';
  let { editor, submitting }: { editor: EditorController; submitting: (id: string) => void } = $props();
  const profiles = new Sbv2Profiles(hostApi), draft = new Sbv2CharacterDraft(hostApi);
  let name = $state(''), credit = $state(''), error = $state('');
  let alive = true;
  const disabled = $derived(draft.busy || editor.busy || !!editor.pending);
  $effect(() => { draft.select(profiles.selectedId); });
  onMount(() => { void profiles.refresh(); });
  onDestroy(() => { alive = false; profiles.dispose(); draft.dispose(); });
  async function add(event: SubmitEvent) {
    event.preventDefault();
    if (disabled || !profiles.ready || !editor.snapshot) return;
    error = '';
    const character = await draft.character(name, credit, editor.snapshot.project.canvas.background);
    if (!character || !alive || editor.busy || editor.pending) return;
    submitting(character.id);
    try { await editor.apply([{ type: 'set_character', character }]); }
    catch (e) { if (alive) error = e && typeof e === 'object' && 'message' in e ? String(e.message) : String(e); }
  }
</script>
<form onsubmit={add} oninput={() => { editor.drafting = true; }}>
  <Sbv2ProfilePicker {profiles} {disabled} />
  <button type="button" disabled={disabled || !profiles.ready} onclick={() => { editor.drafting = true; void draft.inspect(); }}>{locale.text('voiceInspectStyles')}</button>
  {#if draft.busy}<p role="status">{locale.text('voiceInspectBusy')}</p>{/if}
  {#if draft.details}
    <fieldset {disabled}>
      <legend>{draft.details.name}</legend>
      <label>{locale.text('voiceSpeaker')}<select bind:value={draft.speaker}>{#each draft.details.speakers as speaker}<option value={speaker.id}>{speaker.name}</option>{/each}</select></label>
      <label>{locale.text('voiceStyle')}<select bind:value={draft.style}>{#each draft.details.styles as style}<option value={style}>{style}</option>{/each}</select></label>
      <label>{locale.text('voiceLanguage')}<select bind:value={draft.language}>{#each draft.details.languages as language}<option value={language}>{qwenLanguageLabel(language === 'JP' ? 'japanese' : language === 'EN' ? 'english' : 'chinese', locale.current)}</option>{/each}</select></label>
    </fieldset>
  {/if}
  <fieldset {disabled}>
    <label>{locale.text('voiceCharacterName')}<input required bind:value={name} placeholder={locale.text('voiceNameExample')} /></label>
    <label>{locale.text('voiceCredit')}<textarea bind:value={credit} rows="2" placeholder={locale.text('voiceCreditHint')}></textarea></label>
    <p>{locale.text('voiceRegisterHint')}</p>
    <button type="submit" disabled={!draft.details || !profiles.ready}>{locale.text('voiceRegister')}</button>
  </fieldset>
  {#if draft.error}<p role="alert">{draft.error}</p>{/if}
  {#if error}<p role="alert">{error}</p>{/if}
</form>
<style>fieldset { border:1px solid var(--panel-border); border-radius:6px; margin-top:12px; padding:12px; } label { display:flex; flex-direction:column; gap:5px; margin:12px 0; }</style>
