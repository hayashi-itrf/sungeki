# 初回導入（macOS Apple Silicon）

**ビルド済みのSungeki.appが起動できている方は、アプリのビルドは不要です。** 下の「ビルド済みアプリから最初の動画を出す」だけを進めてください。音声エンジンや3D環境を全部入れる必要はありません。

現時点ではターミナルを使う試用版です。動画出力用の環境はまだアプリから自動導入できません。まず文字・図形だけの短い動画で出力を確認し、必要な機能を後から追加します。

## 何が必要か

| やりたいこと | 必要な準備 |
|---|---|
| 起動して文字・図形を編集し、作品を保存する | ビルド済みアプリ。下記の出力用環境は不要 |
| 文字・画像・字幕の動画をMP4にする | 外部ディスクの専用保存先、画像処理用Python環境、FFmpeg／FFprobe、日本語フォントの登録 |
| 合成済み音声のある作品を出力する | 上記と作品が参照する音声・画像などの原本。音声を作り直さなければTTSは不要 |
| セリフを新しく合成する | 上記に加え、使いたい音声エンジンと声だけ |
| VRMを使う | 上記に加え、3D実行環境とモデル。使わなければ準備不要 |
| ソースからアプリを作る | 開発ツールとビルド手順。ビルド済みアプリを使う人は不要 |

## ビルド済みアプリから最初の動画を出す

1. **画像処理環境を一度準備します。** 現行の準備スクリプトには、同じ版のソース一式、Node.js 22.22.3／npm 10.9.8、uv 0.10.6が必要です。下の手順1からNode/npmとuvの案内だけを参照し、[画像処理の手順](../workers/image/README.md#ターミナルで実行する順序)の `setup:image` → `probe:image` を実行してください。Rustの導入、`npm ci`、`npm test`、アプリの再ビルドは不要です。Xcode Command Line ToolsはSungekiのビルドのためには不要ですが、FFmpeg等の導入に使うツールから要求される場合があります。
2. **FFmpegとFFprobeを用意します。** 下の手順3の入手案内を使います。字幕フォントはMacにある利用可能な日本語フォントを選べます。検証ではヒラギノ角ゴシック W3を使っており、フォントの追加購入を必須にはしていません。
3. **アプリの設定に登録します。** 下の手順3の表にある保存先・Python環境・FFmpeg・FFprobe・フォントを登録します。画像処理プログラムは空欄で同梱版を使います。音声エンジンと3Dの欄は空欄のままで構いません。
4. **短い作品を出力します。** 文字または図形を置き、「動画を書き出す」を実行します。「正式な画像を確認」は問題を切り分けるための任意の確認で、動画出力の前に毎回実行する必要はありません。

これは通常は最初の一度だけの準備です。以後、設定先と外部ディスクが利用できれば、動画ごとに再インストールする必要はありません。素材付きの既存作品を別Macで開く場合は、作品JSONだけでなく参照する素材も必要です。元Macの「素材をまとめて保存」を使って持ち出してください。

## 1. ソースと開発ツール（自分でアプリをビルドする場合）

GitHubの「Code」→「Download ZIP」で取得して展開するか、cloneします。以下のコマンドは、展開したフォルダーの `package.json` がある場所で実行してください。ZIPにはGitの履歴がありませんが、ビルドできます。

- Xcode Command Line Tools: `xcode-select --install` で導入し、`xcode-select -p` で導入先を確認します。既に導入済みなら再導入は不要です。
- Node.js 22.22.3 / npm 10.9.8: [Node公式の指定版](https://nodejs.org/en/download/archive/v22.22.3)を使います。ほかの版が既にある場合は、macOS ARM64の圧縮配布物を専用フォルダーに展開し、その `bin` をこのターミナルのPATHの先頭へ追加できます。既存Nodeを削除する必要はありません。
- Rust: [rustup公式手順](https://rust-lang.org/tools/install/)でrustupを用意します。このソース内で `rustup show active-toolchain` を実行すると `rust-toolchain.toml` の指定版が選ばれます。グローバルdefaultを変更しないでください。
- uv 0.10.6: [公式導入手順](https://docs.astral.sh/uv/getting-started/installation/)の特定版の導入を使います。[配布版一覧](https://github.com/astral-sh/uv/releases/tag/0.10.6)からmacOS ARM64版を展開してPATHへ追加する方法もあります。

Nodeを展開した場合の例です。パスは自分の配置先へ置き換えます。ターミナルを閉じると、このPATH変更は引き継がれません。

```sh
export PATH="/Nodeを展開した場所/node-v22.22.3-darwin-arm64/bin:$PATH"
node --version
npm --version
node scripts/check-toolchain.mjs
rustup show active-toolchain
uv --version
```

Nodeは `v22.22.3`、npmは `10.9.8`、uvは `0.10.6` を確認します。確認に失敗した状態で次へ進まず、`command -v node` / `command -v npm` / `command -v uv` で別の版を参照していないか調べてください。

## 2. アプリをビルドする（ビルド済みアプリを使う場合は省略）

```sh
npm ci
npm run check
npm test
npm run desktop:bundle
```

`target/release/bundle/macos/Sungeki.app` ができます。初回は依存のダウンロードとRustのコンパイルがあるため時間がかかります。アプリ・ビルド中間物の保存先にも空き容量が必要です。

このアプリはローカルのアドホック署名です。配布用署名・公証はありません。ビルド成功はすべての外部機能の準備完了を意味しません。

## 3. 最小の動画出力環境

[画像処理の詳細手順](../workers/image/README.md)に従い、同じターミナルで次を実行します。ディスク名は実際のものへ置き換えます。保存先はアプリ専用フォルダーにします。

```sh
export SUNGEKI_RUNTIME_ROOT="/Volumes/動画用SSD/Sungeki/runtime"
npm run setup:image
npm run probe:image
```

probeが `passed` なら画像処理単体の準備は完了です。Pythonはこの専用フォルダーへ導入され、OSのPythonを置き換えません。既存の別環境で成功していても、今回指定したフォルダーのprobeを確認してください。

[FFmpeg公式の入手案内](https://ffmpeg.org/download.html)からmacOS向けFFmpegとFFprobeを用意します。Homebrewを既に使う場合は `brew install ffmpeg` で両方を導入できます。ターミナルで `command -v ffmpeg` / `command -v ffprobe` と、それぞれの `-version` を確認してください。

アプリの「設定」→「動画制作の実行環境」→「保存済み設定を読み込む」を開きます。初回もこのボタンで設定欄を表示します。

| 項目 | 指定内容 |
|---|---|
| 外部ディスクの保存フォルダ | 上の `SUNGEKI_RUNTIME_ROOT` と同じフォルダー |
| Pythonの環境フォルダ | その中の `image/venv` をフォルダー選択 |
| 画像処理プログラム | ビルド済み.appでは空欄（同梱版を使う） |
| FFmpeg / FFprobe | 上で確認した各実行ファイル |
| 字幕フォント | 利用できる日本語フォント。macOS検証ではヒラギノ角ゴシック W3を使用 |
| フォントの検査値 | 通常は手入力不要。フォント選択後の保存時に計算 |

音声エンジン・3D関連の欄は、この段階では空欄で構いません。「設定を保存」後、文字だけの短い作品を新規作成し、「動画を書き出す」を試します。「正式な画像を確認」は任意の事前確認です。「作品を保存」は編集用データの保存であり、MP4出力とは別です。操作は[利用の手引き](USAGE.md)を参照してください。

## 4. 必要な機能を追加する

- 音声: [AivisSpeech / COEIROINKなどの導入・起動設定](LOCAL_SPEECH_ENGINES.md)。全エンジンを入れる必要はありません。
- 3D: [開発ガイド](DEVELOPMENT.md)の専用ブラウザーの準備が必要です。
- 素材・音声・フォント: [第三者の権利](../THIRD_PARTY.md)を確認します。キャラクター素材やモデルは同梱しません。

## よくある停止箇所

| 表示・症状 | 確認すること |
|---|---|
| `npm ci` が `EBADENGINE` | Node/npmの版とPATH。強制オプションで回避せず、手順1へ戻る |
| `command not found` | そのツールの導入先とPATH。ターミナルを開き直すと一時設定は失われる |
| 外部ディスクが未接続 | 実際のマウント名と接続を確認。例の「動画用SSD」をそのまま使わない |
| Pythonのimportエラー | `image/venv` を選んだか確認。OSのPythonやリンク先の基底Pythonを選ばない |
| FFmpegが見つからない | 設定へ絶対パスを指定。Finderから起動したアプリではターミナルのPATHと異なる |
| 音声エンジンへ接続できない | まず公式アプリで声を利用できるか確認。自動起動の欄には.appではなくエンジン実行ファイルを指定 |
| 外部SSDのアクセス確認 | macOSの確認画面で対象アプリとアクセス先を確認して許可する |

新規ソース展開・新規Python環境での試験結果は[実装・検証状況](implementation-status.md)に記載します。既存Mac上の隔離試験と、未設定の別Macでの初回導入は別の確認です。
