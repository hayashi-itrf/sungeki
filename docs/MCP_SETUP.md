# AIからSungeki Makerを使う

Sungeki MakerはMCPを通じたAIの動画制作を中心に、GUIで同じ作品を確認・微調整するアプリです。配布用Sungeki Maker.appにはGUI・MCP・共通ホストが含まれます。MCP用プログラムの別インストールやPython/Node.jsは不要です。

## 一度だけ行う接続設定

1. Sungeki Maker.appを普段使う場所へ置きます。
2. アプリの「使い方」→「AIアプリの接続」→「接続設定を表示」を開きます。
3. AIアプリへ表示された実行ファイルをSTDIO接続として登録します。Codex CLI用の登録コマンドと、mcpServers形式のJSON設定をコピーできます。既存の他の接続設定を上書きしないでください。
4. AIアプリで接続を再読み込みし、Sungeki Makerのツールが利用可能になったことを確認します。

例えば `/Applications/Sungeki Maker.app` に置いた場合、接続先は `/Applications/Sungeki Maker.app/Contents/MacOS/sungeki-mcp` です。同じフォルダの共通ホストを自動で使用するので、通常は環境変数や作業ディレクトリの指定は不要です。アプリを移動した場合は接続先を登録し直してください。GUIを起動したままにする必要はありません。

Codex CLIが導入済みの場合の例:

```sh
codex mcp add sungeki -- "/Applications/Sungeki Maker.app/Contents/MacOS/sungeki-mcp"
codex mcp list
```

## 最初にAIへ頼むこと

「Sungeki Makerのget_capabilitiesとget_runtime_settingsで、利用できる機能と保存済み設定を確認してください。FFmpeg・FFprobe・字幕フォント・作業保存先を設定し、短い試作を出力してください。」

設定保存は `save_runtime_settings` を使います。取得した値をexpectedに指定し、他の編集との競合を検出します。設定保存だけで実行成功とは判断せず、短い動画で検証してください。FFmpeg、TTS、素材の準備やmacOSのアクセス許可はMCP接続とは別です。

保存値と実際の選択値が違いそうな場合は `get_runtime_environment`（protocol 1.160以上、引数なし）を使います。接続中hostの設定保存先と、FFmpeg・FFprobe・字幕フォント・そのSHA256・runtimeルートについて、保存値と環境変数を反映した選択値・出所を確認できます。既存hostに接続した場合は、MCP起動時に渡した環境よりもそのhost自身の環境が使われます。

この取得は読取専用で、ファイル・マウント・フォント照合・実行は未検査（`verification: not_run`）です。VRM・TTS・処理別の作業先、既存ジョブの固定設定は対象外です。選択値が表示されても制作成功の確認にはならず、設定保存やエンジン起動も行いません。応答の型は[RuntimeEnvironment](../crates/protocol/src/runtime_environment.rs)を参照してください。

台本からの制作は `build_video` と `get_build_video`、既存作品は `open_project`、`apply_commands`、`save_project` 等を使用します。GUIとMCPは共通ホストの同じ保存先・作品セッションを使います。既存の作品を調整する際は対象セッションを明示し、別の作品を誤って編集しないでください。

開発用SUNGEKI_HOST_BINARY / SUNGEKI_DATA_DIR / SUNGEKI_RUNTIME_DIRを指定すると、接続先や保存先が変わります。通常利用では指定しません。旧ホストと版が一致しない場合は処理を終えて既存接続を閉じ、新しいアプリで再接続します。

MCPには制作・保存・設定変更などの操作があるため、AIアプリ側の承認設定も確認してください。接続設定の表示・コピーだけではAIアプリの設定を書き換えません。
