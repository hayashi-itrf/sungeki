# 開発とビルド

Tauri v2 / Rust / Svelte / TypeScriptで構成しています。確定した作品状態の変更は共通host/editorが担当し、GUIとMCPは同じコマンドを使います。表示位置・選択・試聴音量はGUIの状態です。

## 固定環境

- Node.js 22.22.3、npm 10.9.8（`.node-version`と`package.json`）
- Rust 1.98.1（`rust-toolchain.toml`）
- 画像処理環境の構築にはuv 0.10.6

macOS Apple Siliconを主な検証対象としています。Linuxのhost/MCPはCI対象ですが、Linux/WindowsのGUIを動作保証するものではありません。

```sh
npm ci
npm run check:toolchain
npm run check
npm test
npm run check:vrm
npm run test:vrm
npm run check:examples
npm run check:structure
npm run build
npm run prepare:host
npm run check:api
cargo check --release --locked
cargo test --release --locked
cargo build --release --locked -p sungeki-mcp
npm run test:host
npm run desktop:bundle
```

`npm run desktop:dev`はreleaseのhostを用意してGUIを起動します。`npm run desktop:dist:bundle`はdist profileでアプリを作成します。`target/debug`は使いません。ローカルのアドホック署名は、Developer ID署名・公証の代わりではありません。

## 実行環境

大きな依存とキャッシュは外部ディスクの専用フォルダーへ置きます。画像処理環境は`workers/image/README.md`を参照してください。VRMは`npm run setup:vrm`で専用ブラウザーを準備します。音声モデル・VRM/PSD・フォント・FFmpegはソースに含みません。

`target/release/sungeki-mcp`はstdio MCPサーバーです。`get_capabilities`で利用可能な機能を確認してください。例は`examples/mcp/README.md`にあります。実行環境が未設定なら音声・映像の一部機能は利用できません。

## 構造と変更時の確認

- `crates/`: Project、編集、素材、音声、描画、host、MCPなどのRustクレート
- `apps/desktop/`: GUIとTauriの接続部分
- `packages/`: API型・クライアントと共通VRM描画
- `workers/`: 外部実行環境で動かす画像・VRM・音声処理
- `tests/`: 自動試験と人工fixture
- `licenses/`: 第三者通知原文と版/hashの対応

固定依存とlockを維持し、`--force`や`--legacy-peer-deps`で互換性確認を回避しないでください。Rust/TSは原則300行、Svelteは250行を目安にし、手書きファイルが400行を超える場合は責務ごとに分割します。

外部エンジン・モデルを使う試験と、モックや人工fixtureの試験は分けて報告します。ビルド済みアプリを配布するときは、実際の同梱物・第三者通知・必要な対象ソース・署名状態を再検査してください。
