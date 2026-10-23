<script lang="ts">
  import GuideDialog from '../../shared/components/GuideDialog.svelte';
  import { locale } from '../../shared/i18n/locale.svelte';
  import McpSetup from '../settings/McpSetup.svelte';
  import KeyboardShortcuts from '../settings/KeyboardShortcuts.svelte';
  let { open = $bindable(false) }: { open?: boolean } = $props();
  let selected = $state('ai');
  const en = $derived(locale.current === 'en');
  const items = $derived([
    {id:'ai',label:en ? 'Create with AI' : 'AIで動画を作る'},
    {id:'connect',label:en ? 'Connect your AI app' : 'AIアプリの接続'},
    {id:'prepare',label:en ? 'Prepare tools and assets' : '必要な準備'},
    {id:'edit',label:en ? 'Review and adjust' : '画面で確認・調整'},
    {id:'keys',label:locale.text('keyboardShortcuts')},
    {id:'trouble',label:en ? 'Troubleshooting' : '困ったときは'},
  ]);
</script>
<GuideDialog bind:open title={en ? 'User guide' : '使い方'} {items} bind:selected>
  <section hidden={selected !== 'ai'}>
    <h2>{en ? 'Create a video with AI' : 'AIで動画を作る'}</h2>
    <p>{en ? 'Sungeki Maker (すんげきメーカー) lets AI create videos through its built-in MCP. Use the editor to review and fine-tune the same project.' : 'Sungeki Maker（すんげきメーカー）は、内蔵MCPを通じてAIが動画を制作し、人が同じ作品を画面で確認・微調整するアプリです。'}</p>
    <ol>
      <li><strong>{en ? 'Connect your AI app once.' : 'AIアプリを一度接続する。'}</strong><p>{en ? 'Install Sungeki Maker in its permanent location, then register it in Codex or another MCP client. No separate MCP installation is needed.' : 'Sungeki Maker.appを普段使う場所に置き、CodexなどのMCP対応AIに登録します。MCPの別インストールは不要です。'}</p><button onclick={() => selected = 'connect'}>{en ? 'Show connection instructions' : '接続手順を見る'}</button></li>
      <li><strong>{en ? 'Ask AI to check the production setup.' : 'AIに制作環境を確認してもらう。'}</strong><p>{en ? 'Register FFmpeg, FFprobe, a subtitle font and a work folder. Prepare a speech engine when using synthesized voices.' : 'FFmpeg・FFprobe・字幕フォント・作業保存先を登録します。合成音声を使う場合は音声エンジンも準備します。設定はAIからも行えます。'}</p></li>
      <li><strong>{en ? 'Provide a script, voices and assets.' : '台本・声・素材と作りたい動画を伝える。'}</strong><p>{en ? 'Specify duration, landscape or portrait, characters, backgrounds and music. Begin with a short test.' : '長さ、横長か縦長か、登場人物、背景、BGMの希望を伝えます。まず短い試作から始めてください。'}</p></li>
      <li><strong>{en ? 'Generate and review.' : 'AIに制作してもらい、確認する。'}</strong><p>{en ? 'Ask AI to save the project and export a video. Check captions, character placement and audio in the result.' : '作品ファイルの保存と動画出力まで依頼します。字幕・立ち絵・音声を確認してください。'}</p></li>
      <li><strong>{en ? 'Fine-tune the same project.' : '同じ作品を微調整する。'}</strong><p>{en ? 'Open the project in Sungeki Maker, adjust it, save, then ask AI to continue with that same project.' : 'Sungeki Makerで作品を開いて調整・保存し、必要ならAIにその作品の続きを依頼します。'}</p></li>
    </ol>
    <h3>{en ? 'Example request to your AI' : 'AIへの依頼例'}</h3>
    <blockquote>{en ? 'Use Sungeki Maker to check available features and saved production settings. Tell me what is missing. Once ready, make a 10-second landscape test from this script, save the project and export the video. Use only the voices and assets I have provided.' : 'Sungeki Makerで使える機能と保存済みの制作環境を確認し、不足しているものを教えてください。準備ができたら、この台本から横長・10秒程度の試作を作り、作品と動画を保存してください。声と素材は私が指定したものを使ってください。'}</blockquote>
    <p>{en ? 'The GUI can be closed during AI production. External tools and asset licenses still need preparation.' : 'AIによる制作中、GUIを開き続ける必要はありません。外部ツールの導入と、使用する声・素材の利用条件の確認は別途必要です。'}</p>
  </section>
  <div hidden={selected !== 'connect'}><McpSetup /></div>
  <section hidden={selected !== 'prepare'}>
    <h2>{en ? 'Prepare only what you use' : '使う機能に必要なものを準備する'}</h2>
    <ul>
      <li>{en ? 'Editing and saving: no speech engine or external renderer required.' : '編集・保存：音声エンジンなどの準備なしで始められます。'}</li>
      <li>{en ? 'Video export: FFmpeg, FFprobe, subtitle font and a work folder.' : '動画出力：FFmpeg・FFprobe・字幕フォント・作業保存先。'}</li>
      <li>{en ? 'Synthesized speech: install the engine you want, and prepare its voices or models. You do not need every supported engine.' : '音声合成：使いたい音声エンジンと声・モデル。対応エンジンを全部入れる必要はありません。'}</li>
      <li>{en ? '3D characters: select your VRM model and motion. Rendering tools are included in the app.' : '3Dキャラクター：VRMモデルとモーションを選びます。描画ツールはアプリに含まれています。'}</li>
    </ul>
    <p>{en ? 'Paths are configured under Settings → Production environment, or through MCP. Saving paths does not prove export works; try a short video.' : 'ファイルの場所は「設定」→「動画制作の実行環境」、またはMCPから登録できます。保存できたことと出力できることは別なので、短い動画で動作を確認してください。'}</p>
  </section>
  <section hidden={selected !== 'edit'}>
    <h2>{en ? 'Review and adjust in the editor' : '画面で確認・調整する'}</h2>
    <ol>
      <li>{en ? 'Open the project saved by AI using Project / File, or connect to its existing session.' : '「作品・ファイル」からAIが保存した作品を開くか、復旧済みの対象作品へ接続します。'}</li>
      <li>{en ? 'Select a timeline clip and adjust its appearance in the right panel.' : 'タイムラインでクリップを選び、右側で見た目を調整します。'}</li>
      <li>{en ? 'Appearance values apply when confirmed. Text, timing and related fields use Apply changes.' : '見た目の値は入力確定時に反映します。本文・時刻などは「変更を適用」でまとめて確定します。'}</li>
      <li>{en ? 'Space plays or pauses outside text fields. The editing preview can differ from final output; check the exported video.' : '文字入力欄以外でSpaceを押すと再生・一時停止します。編集用プレビューと正式出力には差があるため、出力した動画も確認してください。'}</li>
      <li>{en ? 'Save project saves editable project data; exporting creates the video file.' : '「作品を保存」は編集用データの保存です。動画ファイルは「動画を書き出す」で作成します。'}</li>
    </ol>
  </section>
  <div hidden={selected !== 'keys'}><KeyboardShortcuts /></div>
  <section hidden={selected !== 'trouble'}>
    <h2>{en ? 'Troubleshooting' : '困ったときは'}</h2>
    <h3>{en ? 'AI cannot find Sungeki Maker' : 'AIからSungeki Makerが見つからない'}</h3>
    <p>{en ? 'Check the registered executable path. If you moved the app, show the connection settings again and update the AI client. Reload its MCP connections.' : '登録した実行ファイルの場所を確認します。アプリを移した場合は「AIアプリの接続」で現在の場所を表示し、AI側の登録を更新して接続を再読み込みしてください。'}</p>
    <h3>{en ? 'Export or synthesis fails' : '動画出力・音声合成に失敗する'}</h3>
    <p>{en ? 'Check the error and the configured files, storage availability, engine setup and macOS access permissions. Settings → Connection diagnostics shows host connection information.' : 'エラー内容、設定したファイルの場所、保存先の空き容量、音声エンジン、macOSのアクセス許可を確認します。「設定」→「接続・起動の確認」で接続情報も確認できます。'}</p>
    <h3>{en ? 'AI and GUI show different projects' : 'AIと画面で違う作品が表示される'}</h3>
    <p>{en ? 'Confirm the same project file and session are selected. Save or apply pending changes before asking AI to continue.' : '同じ作品ファイル・セッションを選んでいるか確認してください。画面の未適用入力を確定し、保存してからAIに続きを依頼します。'}</p>
  </section>
</GuideDialog>
<style>
  li { margin-bottom:14px; } li p { margin:5px 0; }
  blockquote { margin:12px 0; padding:14px; border-left:3px solid var(--accent,#315fff); background:var(--surface); }
</style>
