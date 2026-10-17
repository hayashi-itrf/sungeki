<script lang="ts">
  import { characterText as t, characterDiagnostic } from './character-messages';
  import { onDestroy } from 'svelte';
  import type { VoicevoxEngineInfo, VoicevoxSpeakerInfo, VoicevoxSpeakerDetails } from '@sungeki/api-client';
  import { hostApi } from '../../shared/host-api';
  import { voicevoxUrl, rememberVoicevoxUrl } from './connection';
  export type VoiceChoice = { baseUrl: string; details: VoicevoxSpeakerDetails; styleId: number };
  let { choose, disabled = false }: { choose: (value: VoiceChoice | null) => void; disabled?: boolean } = $props();
  let baseUrl = $state(voicevoxUrl());
  let engine = $state<VoicevoxEngineInfo | null>(null);
  let core = $state(''), speakerId = $state(''), styleId = $state('');
  let speakers = $state<VoicevoxSpeakerInfo[]>([]);
  let details = $state<VoicevoxSpeakerDetails | null>(null);
  let busy = $state(false), error = $state('');
  let alive = true;
  onDestroy(() => { alive = false; });
  function reset(scope: 'engine' | 'core' | 'speaker') {
    details = null; choose(null); error = ''; styleId = '';
    if (scope !== 'speaker') { speakers = []; speakerId = ''; }
    if (scope === 'engine') { engine = null; core = ''; }
  }
  async function run(action: () => Promise<void>) {
    if (busy || disabled) return;
    busy = true; error = ''; choose(null); details = null;
    try { await action(); }
    catch (e) { if (alive) error = e && typeof e === 'object' && 'message' in e ? String(e.message) : String(e); }
    finally { if (alive) busy = false; }
  }
  async function discover() {
    reset('engine');
    await run(async () => {
      const result = await hostApi.session({ type: 'discover_voicevox', args: { base_url: baseUrl } });
      if (!alive) return;
      if (result.type !== 'voicevox_engine') throw new Error('接続情報の応答が不正です。');
      engine = result.engine;
      rememberVoicevoxUrl(baseUrl);
      if (engine.core_versions.length === 1) { core = engine.core_versions[0]!; await catalog(); }
    });
  }
  async function catalog() {
    if (!engine || !core) return;
    const result = await hostApi.session({ type: 'get_voicevox_catalog', args: { base_url: baseUrl, expected_engine: engine, core_version: core } });
    if (!alive) return;
    if (result.type !== 'voicevox_catalog') throw new Error('声一覧の応答が不正です。');
    speakers = result.speakers;
    if (!speakers.length) error = 'このエンジンには利用できる読み上げ話者がありません。';
  }
  async function loadDetails() {
    if (!engine || !speakerId) return;
    await run(async () => {
      const result = await hostApi.session({ type: 'get_voicevox_speaker', args: { base_url: baseUrl, expected_engine: engine!, core_version: core, speaker_uuid: speakerId } });
      if (!alive) return;
      if (result.type !== 'voicevox_speaker') throw new Error('話者情報の応答が不正です。');
      details = result.details; styleId = String(details.speaker.styles[0]?.id ?? ''); publish();
    });
  }
  function publish() {
    const style = details?.speaker.styles.find(s => String(s.id) === styleId);
    choose(details && style ? { baseUrl, details, styleId: style.id } : null);
  }
</script>
<fieldset disabled={disabled || busy}>
  <p>{t("起動済みのVOICEVOXに接続して声を選びます。登録後にセリフを追加し、「音声合成」から生成できます。エンジンは先に起動してください。")}</p>
  <details><summary>{t("エンジンの接続先")}</summary><label>{t("接続先")}<input bind:value={baseUrl} oninput={() => reset('engine')} /></label></details>
  <button type="button" onclick={discover}>{t("声の一覧を取得")}</button>
  {#if engine}
    <p>{engine.name} — {engine.version}</p>
    {#if engine.core_versions.length > 1}
      <label>{t("音声エンジンのバージョン")}<select bind:value={core} onchange={() => { reset('core'); void run(catalog); }}><option value="">{t("選択してください")}</option>{#each engine.core_versions as value}<option value={value}>{value}</option>{/each}</select></label>
    {/if}
    <label>{t("話者")}<select bind:value={speakerId} onchange={() => reset('speaker')}><option value="">{t("選択してください")}</option>{#each speakers as speaker}<option value={speaker.speaker_uuid}>{speaker.name}</option>{/each}</select></label>
    <button type="button" disabled={!speakerId} onclick={loadDetails}>{t("話者の詳細・利用条件を取得")}</button>
  {/if}
  {#if details}
    <label>{t("スタイル")}<select bind:value={styleId} onchange={publish}>{#each details.speaker.styles as style}<option value={String(style.id)}>{style.name}</option>{/each}</select></label>
    <details><summary>{t("エンジンの利用条件")}</summary><pre>{details.engine_terms || t("利用条件が提供されていません。配布元で確認してください。")}</pre></details>
    <details><summary>{t("話者の利用条件")}</summary><pre>{details.speaker_policy || t("利用条件が提供されていません。配布元で確認してください。")}</pre></details>
  {/if}
</fieldset>
{#if busy}<p role="status">{t("声の情報を取得しています…")}</p>{/if}
{#if error}<p role="alert">{characterDiagnostic(error)}</p>{/if}
<style>
  fieldset { border:0; padding:0; } label { display:flex; flex-direction:column; gap:5px; margin:12px 0; }
  pre { white-space:pre-wrap; overflow-wrap:anywhere; max-height:220px; overflow:auto; font:inherit; padding:10px; border:1px solid var(--panel-border); }
</style>
