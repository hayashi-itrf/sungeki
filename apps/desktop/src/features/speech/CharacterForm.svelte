<script lang="ts">
  import { characterText as t } from './character-messages';
  import type { EditorController } from '../editor/controller.svelte';
  import LocalCharacterForm from './LocalCharacterForm.svelte';
  import VoicevoxCharacterForm from './VoicevoxCharacterForm.svelte';
  import IrodoriCharacterForm from './IrodoriCharacterForm.svelte';
  import QwenCharacterForm from './QwenCharacterForm.svelte';
  import Sbv2CharacterForm from './Sbv2CharacterForm.svelte';
  let { editor, submitting }: { editor: EditorController; submitting: (id: string) => void } = $props();
  let engine = $state('voicevox');
</script>
<label>{t("音声エンジン")}
  <select bind:value={engine} disabled={editor.busy || !!editor.pending}>
    <option value="voicevox">VOICEVOX</option><option value="aivis_speech">AivisSpeech</option><option value="coeiroink">COEIROINK</option><option value="sbv2">Style-Bert-VITS2</option><option value="qwen">Qwen</option><option value="irodori">Irodori</option>
  </select>
</label>
{#if engine === 'aivis_speech' || engine === 'coeiroink'}{#key engine}<LocalCharacterForm {editor} {submitting} {engine} />{/key}{:else if engine === 'irodori'}<IrodoriCharacterForm {editor} {submitting} />{:else if engine === 'qwen'}<QwenCharacterForm {editor} {submitting} />{:else if engine === 'sbv2'}<Sbv2CharacterForm {editor} {submitting} />{:else}<VoicevoxCharacterForm {editor} {submitting} />{/if}
<style>label { display:flex; flex-direction:column; gap:6px; margin-bottom:16px; }</style>
