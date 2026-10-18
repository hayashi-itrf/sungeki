<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
  import type { Character, LocalSpeechEngine, LocalSpeechCatalog, LocalVoiceSettings } from '@sungeki/api-client';
  import type { EditorController } from '../editor/controller.svelte';
  import { hostApi } from '../../shared/host-api';
  import { newCharacterSubtitles } from './new-character-subtitles';
  import { localEndpoint, saveLocalEndpoint } from './local-connection';
  import { locale } from '../../shared/i18n/locale.svelte';
  let { editor, submitting, engine }: { editor: EditorController; submitting: (id: string) => void; engine: LocalSpeechEngine } = $props();
  const text = (ja: string, en: string) => locale.current === 'ja' ? ja : en;
  let endpoint = $state(untrack(() => localEndpoint(engine))), catalog = $state<LocalSpeechCatalog | null>(null);
  let selection = $state(''), name = $state(''), credit = $state(''), error = $state(''), busy = $state(false);
  let policy = $state('');
  let alive = true;
  onDestroy(() => { alive = false; });
  const label = $derived(engine === 'aivis_speech' ? 'AivisSpeech' : 'COEIROINK');
  async function load() {
    if (busy) return;
    busy = true; error = ''; catalog = null; selection = '';
    policy = '';
    const captured = endpoint;
    try {
      const response = await hostApi.session({ type: 'get_local_speech_catalog', args: { base_url: captured, engine } });
      if (!alive || captured !== endpoint) return;
      if (response.type !== 'local_speech_catalog' || response.catalog.engine !== engine) throw new Error('話者一覧の応答が一致しません');
      catalog = response.catalog; saveLocalEndpoint(engine, captured);
    } catch (e) { if (alive) error = e instanceof Error ? e.message : String(e); }
    finally { if (alive) busy = false; }
  }
  async function selectVoice() {
    policy = ''; error = '';
    const captured = selection, url = endpoint;
    const speaker_uuid = selection.split(':')[0];
    if (!speaker_uuid) return;
    busy = true;
    try {
      const result = await hostApi.session({ type: 'get_local_speech_policy', args: { engine, base_url: url, speaker_uuid } });
      if (!alive || captured !== selection || url !== endpoint) return;
      if (result.type !== 'local_speech_policy') throw new Error('利用条件の応答が不正です');
      policy = result.policy;
    } catch (e) { if (alive) error = e instanceof Error ? e.message : String(e); }
    finally { if (alive) busy = false; }
  }
  async function add(event: SubmitEvent) {
    event.preventDefault();
    if (!catalog || !policy || busy || editor.busy || editor.pending) return;
    const chosen = catalog.speakers.flatMap(s => s.styles.map(style => ({ speaker: s, style }))).find(s => `${s.speaker.speaker_uuid}:${s.style.id}` === selection);
    if (!chosen || !name.trim()) return;
    const common = { speaker_uuid: chosen.speaker.speaker_uuid, style_id: chosen.style.id, speed_scale: 1, pitch_scale: 0, volume_scale: 1, pre_phoneme_length: .1, post_phoneme_length: .1 };
    const settings: LocalVoiceSettings = engine === 'aivis_speech' ? { engine, ...common, emotion_scale: 1, tempo_dynamics_scale: 1 } : { engine, ...common, intonation_scale: 1 };
    const character: Character = { id: crypto.randomUUID(), name: name.trim(), credit: credit.trim() || `${label}: ${chosen.speaker.name}`, voice: { type: 'local_http', settings }, subtitle_style: newCharacterSubtitles(editor.snapshot!.project.canvas.background) };
    submitting(character.id);
    await editor.apply([{ type: 'set_character', character }]);
  }
</script>
<form onsubmit={add} oninput={() => { editor.drafting = true; }}>
  <fieldset disabled={busy || editor.busy || !!editor.pending}>
    <label>{label} URL<input bind:value={endpoint} oninput={() => { catalog = null; selection = ''; policy = ''; }} /></label>
    <button type="button" onclick={load}>{text('話者一覧を取得', 'Load voices')}</button>
    <p>{text('自動起動を設定済みなら「話者一覧を取得」で起動します。未設定なら先にエンジンを起動してください。声の利用条件も確認してください。', 'Loading voices starts a configured engine automatically. Otherwise start it first. Check the voice terms.')}</p>
    <a href={engine === 'aivis_speech' ? 'https://hub.aivis-project.com/' : 'https://coeiroink.com/terms'} target="_blank" rel="noreferrer">{text('利用条件を確認', 'View terms')}</a>
    <label>{text('声', 'Voice')}<select bind:value={selection} onchange={selectVoice} required><option value="">{text('選択してください', 'Select a voice')}</option>
      {#each catalog?.speakers ?? [] as speaker}{#each speaker.styles as style}<option value={`${speaker.speaker_uuid}:${style.id}`}>{speaker.name} / {style.name}</option>{/each}{/each}
    </select></label>
    {#if policy}<details open><summary>{text('選択した声の利用条件', 'Selected voice terms')}</summary><pre>{policy}</pre></details>{/if}
    <label>{text('作品内のキャラクター名', 'Character name')}<input bind:value={name} required /></label>
    <label>{text('クレジット', 'Credit')}<input bind:value={credit} placeholder={`${label}: ${text('話者名', 'speaker name')}`} /></label>
    <button type="submit" disabled={!selection || !catalog || !policy}>{text('キャラクターを登録', 'Add character')}</button>
  </fieldset>
  {#if error}<p role="alert">{error}</p>{/if}
</form>
<style>pre { white-space:pre-wrap; overflow-wrap:anywhere; max-height:180px; overflow:auto; } fieldset { border:0; padding:0; } label { display:flex; flex-direction:column; gap:5px; margin:12px 0; }</style>
