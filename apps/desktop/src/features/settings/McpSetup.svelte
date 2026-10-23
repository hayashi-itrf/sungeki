<script lang="ts">
  import { invoke, isTauri } from '@tauri-apps/api/core';
  import { locale } from '../../shared/i18n/locale.svelte';
  let path = $state('');
  let error = $state('');
  let busy = $state(false);
  let copied = $state(false);
  const en = $derived(locale.current === 'en');
  const command = $derived(`codex mcp add sungeki -- '${path.replaceAll("'", "'\\''")}'`);
  const config = $derived(JSON.stringify({ mcpServers: { sungeki: { command: path, args: [] } } }, null, 2));
  async function load() {
    busy = true; error = ''; copied = false;
    try {
      if (!isTauri()) throw new Error(en ? 'Open the native app to get its location.' : 'アプリから開いて接続先を確認してください。');
      path = await invoke<string>('mcp_executable_path');
    } catch (e) { error = String(e); }
    finally { busy = false; }
  }
  async function copy(value: string) {
    copied = false; error = '';
    try { await navigator.clipboard.writeText(value); copied = true; }
    catch { error = en ? 'Select and copy the text below.' : '下のテキストを選択してコピーしてください。'; }
  }
</script>

<section>
  <h2>{en ? 'Connect your AI app' : 'AIアプリを接続する'}</h2>
  <p>{en ? 'MCP is included in this app. AI can configure production, edit scripts and export videos. The GUI shares the same projects for review and adjustments.' : 'MCPはこのアプリに内蔵されています。AIから制作環境の設定・台本編集・動画出力ができ、同じ作品を画面で確認・調整できます。'}</p>
  <p>{en ? 'Move Sungeki Maker.app to its permanent location first. Register the connection in your AI client once. Moving the app later requires updating the path. The GUI does not need to stay open.' : '先にSungeki Maker.appを普段使う場所へ移し、AIアプリに接続先を一度登録してください。後からアプリを移した場合は登録先を更新します。GUIを開き続ける必要はありません。'}</p>
  <button onclick={load} disabled={busy}>{en ? 'Show connection settings' : '接続設定を表示'}</button>
  {#if path}
    <p>{en ? 'MCP executable (STDIO)' : 'MCP実行ファイル（STDIO）'}</p>
    <pre>{path}</pre>
    <p>{en ? 'For Codex CLI: run this command in Terminal. It registers the connection; it does not install Codex.' : 'Codex CLIを使う場合：次のコマンドをターミナルで実行します。接続の登録用で、Codex自体のインストールは行いません。'}</p>
    <pre>{command}</pre>
    <button onclick={() => copy(command)}>{en ? 'Copy Codex command' : 'Codex用コマンドをコピー'}</button>
    <p>{en ? 'For clients using mcpServers JSON: merge this entry into the existing configuration.' : 'mcpServers形式のJSONを使うAIアプリ：既存設定を消さず、以下の接続項目を追加してください。'}</p>
    <pre>{config}</pre>
    <button onclick={() => copy(config)}>{en ? 'Copy JSON settings' : 'JSON設定をコピー'}</button>
    <p>{en ? 'Reload connections in your AI client. Ask it to check Sungeki Maker capabilities and production settings first. External tools, voices and assets still need preparation.' : '登録後はAIアプリで接続を再読み込みし、まず「Sungeki Makerの機能と制作環境の設定を確認して」と頼んでください。FFmpeg・音声エンジン・素材の準備は別途必要です。'}</p>
  {/if}
  {#if copied}<p role="status">{en ? 'Copied.' : 'コピーしました。'}</p>{/if}
  {#if error}<p role="alert">{error}</p>{/if}
</section>
<style>
  pre { white-space:pre-wrap; overflow-wrap:anywhere; user-select:text; padding:10px; border:1px solid var(--border); border-radius:6px; }
</style>
