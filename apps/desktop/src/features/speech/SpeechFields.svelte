<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { Character } from '@sungeki/api-client';
  let { characters, characterId = $bindable(), subtitle = $bindable(), spoken = $bindable(), explicit = $bindable() }: {
    characters: Character[]; characterId: string; subtitle: string; spoken: string; explicit: boolean;
  } = $props();
</script>
<p>{locale.text('speechFieldsHint')}</p>
<label>{locale.text('speechCharacter')}<select bind:value={characterId}>{#each characters as character}<option value={character.id}>{character.name}</option>{/each}</select></label>
<label>{locale.text('speechSubtitle')}<textarea bind:value={subtitle} rows="3"></textarea></label>
<p>{locale.text('speechSubtitleHint')}</p>
<label class="toggle"><input type="checkbox" bind:checked={explicit} />{locale.text('speechExplicitReading')}</label>
{#if explicit}<label>{locale.text('speechSpokenText')}<textarea bind:value={spoken} rows="3" required></textarea></label>{:else}<p>{locale.text('speechDictionaryHint')}</p>{/if}
<style>
  label { display:flex; flex-direction:column; gap:4px; margin-bottom:10px; }
  .toggle { flex-direction:row; align-items:center; }
  textarea, select { width:100%; }
  p { font-size:12px; }
</style>
