<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import type { ManagedSpeechEngine } from '@sungeki/api-client';
  import { locale } from '../../shared/i18n/locale.svelte';
  import { hostApi } from '../../shared/host-api';
  let engine = $state<ManagedSpeechEngine>('sbv2');
  let path = $state(''), busy = $state(false), result = $state(''), error = $state('');
  const en = $derived(locale.current === 'en');
  async function choose() {
    try {
      const selected = await open({ multiple: false, filters: [{ name: 'JSON', extensions: ['json'] }] });
      if (typeof selected === 'string') { path = selected; result = ''; error = ''; }
    } catch (value) { error = String(value); }
  }
  async function install() {
    if (busy || !path) return;
    busy = true; result = ''; error = '';
    try {
      const response = await hostApi.session({ type: 'install_speech_config', args: { engine, candidate_path: path } });
      if (response.type !== 'speech_config_installed') throw new Error(en ? 'Unexpected registration response.' : '登録の応答を確認できませんでした。');
      result = response.configuration_path;
    } catch (value) {
      error = value && typeof value === 'object' && 'message' in value ? String(value.message) : String(value);
    } finally { busy = false; }
  }
</script>
<details>
  <summary>{en ? 'Register a voice engine configuration' : '音声エンジンの起動設定を登録'}</summary>
  <p>{en ? 'Select a prepared Sungeki configuration JSON. The app checks its declared model originals before registering it. Existing different settings are never overwritten. This does not download or start a model.' : 'Sungeki用に準備した設定JSONを選びます。登録前にモデル原本を検査し、異なる既存設定は上書きしません。モデルのダウンロードや起動は行いません。'}</p>
  <fieldset disabled={busy}>
    <label>{en ? 'Engine' : '音声エンジン'}
      <select bind:value={engine} onchange={() => { path = ''; result = ''; error = ''; }}><option value="sbv2">SBV2</option><option value="qwen">Qwen</option><option value="irodori">Irodori</option></select>
    </label>
    <button onclick={choose}>{en ? 'Choose configuration JSON' : '設定JSONを選ぶ'}</button>
    {#if path}<code>{path}</code>{/if}
    <button disabled={!path} onclick={install}>{en ? 'Check and register' : '検査して登録'}</button>
  </fieldset>
  {#if busy}<p role="status">{en ? 'Checking originals. This may take several minutes. Closing settings does not cancel registration. Cancellation from this screen is not yet supported.' : '原本を確認中です。数分かかる場合があります。設定画面を閉じても登録は継続します。この画面からの取消はまだ未対応です。'}</p>{/if}
  {#if result}<p role="status">{en ? 'Registered. Refresh profiles in the speech dialog. Synthesis has not been tested.' : '登録しました。音声画面の「設定一覧を更新」を押してください。実合成はまだ確認していません。'}</p><code>{result}</code>{/if}
  {#if error}<p role="alert">{error}</p><p>{en ? 'If the response was lost, retry the same configuration. Identical settings are accepted without duplication.' : '応答を確認できなかった場合は、同じ設定で再確認してください。同一内容の再登録は重複を作りません。'}</p>{/if}
</details>
<style>
  fieldset { display:grid; gap:10px; border:1px solid var(--border); margin:12px 0; }
  label { display:grid; gap:5px; } code { overflow-wrap:anywhere; } button { justify-self:start; }
</style>
