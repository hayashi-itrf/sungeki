<script lang="ts">
  import LocalTuningFields from "./LocalTuningFields.svelte";
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { tuningText as t } from './tuning-messages';
  import { onDestroy } from 'svelte';
  import { captureVoiceTuning } from './voice-tuning-draft.svelte';
  import type { Character, SessionSnapshot } from '@sungeki/api-client';
  import type { EditorController } from '../editor/controller.svelte';
  import { hostApi } from '../../shared/host-api';
  import { EditPlan } from '../scenes/edit-plan.svelte';
  import EditReview from '../scenes/EditReview.svelte';
  import EditorNotices from '../editor/EditorNotices.svelte';
  import Sbv2TuningFields from './Sbv2TuningFields.svelte';
  import { tuneSbv2 } from './sbv2-tuning';
  import { tuneVoicevox } from './voicevox-tuning';
  import IrodoriTuningFields from './IrodoriTuningFields.svelte';
  import { tuneIrodori } from './irodori-tuning';
  import QwenTuningFields from './QwenTuningFields.svelte';
  import VoicevoxTuningFields from './VoicevoxTuningFields.svelte';
  let { editor }: { editor: EditorController } = $props();
  let dialog: HTMLDialogElement;
  let observed = $state.raw<SessionSnapshot | null>(null);
  let characterId = $state(''), error = $state('');
  let voice = $state<Character['voice'] | null>(null);
  const plan = new EditPlan(hostApi);
  const characters = $derived(observed?.project.characters ?? []);
  const stale = $derived(!!observed && (observed.session_id !== editor.snapshot?.session_id || observed.revision !== editor.snapshot?.revision));
  const disabled = $derived(stale || editor.busy || !!editor.pending || editor.drafting);
  onDestroy(() => plan.cancel());
  function choose() {
    plan.cancel(); error = '';
    const selected = characters.find(c => c.id === characterId)?.voice;
    voice = selected ? structuredClone(selected) : null;
  }
  function refresh() {
    observed = captureVoiceTuning(editor.snapshot);
    characterId = observed?.project.characters[0]?.id ?? '';
    choose();
  }
  async function preview(event: SubmitEvent) {
    event.preventDefault(); error = '';
    const character = characters.find(c => c.id === characterId);
    if (disabled || plan.busy || !observed || !character || !voice) return;
    try { await plan.prepare(observed, [voice.type === 'irodori' ? tuneIrodori(character, voice.settings) : (voice.type === 'qwen' || voice.type === 'local_http') ? { type: 'set_character', character: structuredClone({ ...character, voice }) } : voice.type === 'sbv2' ? tuneSbv2(character, voice.settings) : tuneVoicevox(character, voice.settings)]); }
    catch (e) { error = e instanceof Error ? e.message : String(e); }
  }
  async function confirm() {
    if (disabled || plan.busy || plan.stale(editor.snapshot) || !plan.request || !plan.preview || plan.preview.no_op || plan.error) return;
    await editor.request(plan.request);
    if (!editor.error && !editor.pending) { plan.cancel(); dialog.close(); }
  }
</script>
<button disabled={editor.busy || !!editor.pending || editor.drafting || !editor.snapshot?.project.characters.length} onclick={() => { refresh(); dialog.showModal(); }}>{t("声を調整")}</button>
<dialog use:containDialogFocus bind:this={dialog} aria-label={t("声を調整")} onclose={() => plan.cancel()} oncancel={e => { if (editor.busy || editor.pending) e.preventDefault(); }}>
  <header><h2>{t("声を調整")}</h2><button disabled={editor.busy || !!editor.pending} onclick={() => dialog.close()}>{t("閉じる")}</button></header>
  <EditorNotices {editor} />
  <p>{t("同じキャラクターのセリフすべてに適用します。変更した場合は生成済み音声が外れ、音声合成とセリフの再配置が必要です。元の音声素材は残り、「元に戻す」で設定と音声を戻せます。")}</p>
  {#if stale}<p role="alert">{t("作品が更新されました。最新の状態を読み込んでください。")}</p>{/if}
  <button disabled={editor.busy || !!editor.pending || editor.drafting} onclick={refresh}>{t("入力を破棄して最新の状態を読む")}</button>
  {#if observed && plan.request}
    <EditReview {plan} project={observed.project} stale={stale || plan.stale(editor.snapshot)} {disabled} {confirm} />
  {:else}
    <form onsubmit={preview}><fieldset disabled={disabled || plan.busy}>
      <label>{t("キャラクター")}<select bind:value={characterId} onchange={choose}>{#each characters as character}<option value={character.id}>{character.name}（{{ voicevox: 'VOICEVOX', sbv2: 'SBV2', qwen: 'Qwen', irodori: 'Irodori', local_http: character.voice.type === 'local_http' ? (character.voice.settings.engine === 'aivis_speech' ? 'AivisSpeech' : 'COEIROINK') : '' }[character.voice.type]}）</option>{/each}</select></label>
      {#if voice?.type === 'local_http'}<LocalTuningFields bind:tuning={voice.settings} />{:else if voice?.type === 'irodori'}<IrodoriTuningFields bind:tuning={voice.settings} />{:else if voice?.type === 'qwen'}<QwenTuningFields bind:tuning={voice.settings} />{:else if voice?.type === 'sbv2'}<Sbv2TuningFields bind:tuning={voice.settings} />{:else if voice?.type === 'voicevox'}<VoicevoxTuningFields bind:tuning={voice.settings} />{/if}
      <button disabled={!voice}>{t("変更内容を確認")}</button>
    </fieldset></form>
  {/if}
  {#if error}<p role="alert">{error}</p>{/if}
</dialog>
<style>header { display:flex; justify-content:space-between; align-items:center; gap:24px; } dialog { max-width:700px; } fieldset { border:0; padding:12px 0; } label { display:block; margin:12px 0; }</style>
