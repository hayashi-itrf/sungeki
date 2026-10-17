<script lang="ts">
  import { characterText as t, characterDiagnostic } from './character-messages';
  import { onDestroy } from 'svelte';
  import type { EditorController } from '../editor/controller.svelte';
  import { hostApi } from '../../shared/host-api';
  import VoicePicker, { type VoiceChoice } from './VoicePicker.svelte';
  import { createCharacter } from './create-character';
  let { editor, submitting }: { editor: EditorController; submitting: (id: string) => void } = $props();
  let choice = $state<VoiceChoice | null>(null), name = $state(''), credit = $state('');
  let checking = $state(false), error = $state('');
  let alive = true;
  onDestroy(() => { alive = false; });
  async function add(event: SubmitEvent) {
    event.preventDefault();
    if (!choice || checking || editor.busy || editor.pending) return;
    checking = true; error = '';
    const captured = choice;
    try {
      const result = await hostApi.session({ type: 'get_voicevox_speaker', args: {
        base_url: captured.baseUrl, expected_engine: captured.details.engine,
        core_version: captured.details.core_version, speaker_uuid: captured.details.speaker.speaker_uuid,
      } });
      if (!alive) return;
      if (result.type !== 'voicevox_speaker' || JSON.stringify(result.details) !== JSON.stringify(captured.details)) {
        choice = null; throw new Error('声の情報または利用条件が変わりました。一覧から選び直してください。');
      }
      const character = createCharacter(result.details, captured.styleId, name, credit, editor.snapshot!.project.canvas.background);
      submitting(character.id);
      await editor.apply([{ type: 'set_character', character }]);
    } catch (e) { if (alive) error = e && typeof e === 'object' && 'message' in e ? String(e.message) : String(e); }
    finally { if (alive) checking = false; }
  }
</script>
<form onsubmit={add} oninput={() => { editor.drafting = true; }}>
  <VoicePicker disabled={checking || editor.busy || !!editor.pending} choose={value => { choice = value; editor.drafting = true; }} />
  <fieldset disabled={checking || editor.busy || !!editor.pending}>
    <label>{t("作品内のキャラクター名")}<input required bind:value={name} placeholder={t("例：ナレーター")} /></label>
    <label>{t("クレジット")}<textarea bind:value={credit} rows="2" placeholder={t("利用条件を確認して記入してください")}></textarea></label>
    <p>{t("標準の話速・音量で登録します。字幕は現在の背景色に合わせて黒か白を選びます。背景画像や後からの背景変更には自動追従しません。登録だけでは音声は生成されません。")}</p>
    <button type="submit" disabled={!choice}>{checking ? t("声の情報を再確認中…") : t("キャラクターを登録")}</button>
  </fieldset>
  {#if error}<p role="alert">{characterDiagnostic(error)}</p>{/if}
</form>
<style>fieldset { border:0; padding:0; } label { display:flex; flex-direction:column; gap:5px; margin:12px 0; }</style>
