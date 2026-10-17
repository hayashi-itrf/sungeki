<script lang="ts">
  import type { PresetRecord } from '@sungeki/api-client';
  import type { EditorController } from '../editor/controller.svelte';
  import { locale } from '../../shared/i18n/locale.svelte';
  let { editor, record, locked, onapply }: { editor: EditorController; record: PresetRecord; locked: boolean; onapply: () => void } = $props();
  const t = (ja: string, en: string) => locale.current === 'en' ? en : ja;
  let characterId = $state(''), credit = $state(''), notice = $state('');
  const required = $derived(record.preset?.content.kind === 'voice' ? record.preset.content.credit : '');
  function selectCharacter() {
    const current = editor.snapshot?.project.characters.find(c => c.id === characterId)?.credit ?? '';
    credit = current.includes(required) ? current : [current, required].filter(Boolean).join('\n');
    notice = '';
  }
  async function apply() {
    if (!record.preset || locked || !characterId) return;
    notice = ''; onapply();
    await editor.apply([{ type: 'apply_voice_preset', preset_id: record.id, preset_revision: record.revision,
      preset: record.preset, character_id: characterId, credit }]);
    notice = editor.error ? '' : t('声を適用しました。声が変わった人物のセリフは再合成してください。Undoで戻せます。', 'Voice applied. Resynthesize speech for characters whose voice changed. You can undo this.');
  }
</script>
<p>{required}</p>
<label>{t('声を変更する人物', 'Character to change')}<select bind:value={characterId} onchange={selectCharacter}><option value="">{t('選択してください', 'Select')}</option>{#each editor.snapshot?.project.characters ?? [] as c}<option value={c.id}>{c.name}</option>{/each}</select></label>
<label>{t('適用後のクレジット', 'Credits after applying')}<textarea bind:value={credit} rows="4" maxlength="65536"></textarea></label>
<p>{t('立ち絵などのクレジットを残し、新しい声の表記を含めてください。名前・字幕・立ち絵の設定は保持します。', 'Keep artwork credits and include the new voice attribution. Name, subtitles and standing settings are preserved.')}</p>
<button disabled={locked || !characterId || !credit.includes(required)} onclick={apply}>{t('この人物に声を適用', 'Apply voice to this character')}</button>
{#if notice}<p role="status">{notice}</p>{/if}
<style>label { display:grid; gap:4px; margin:10px 0; }</style>
