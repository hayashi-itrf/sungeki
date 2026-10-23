# VRM描画環境の同梱

macOS配布アプリは、Node.js、Playwright、描画専用Chromium Headless Shell、共有VRM描画コードを `Contents/Resources/vrm-runtime` に含めます。モデル・モーション素材は含めません。通常の動画出力用の保存先・FFmpeg・FFprobe・フォントに加えて、VRM用ツールを利用者が設定する必要はありません。

GUIとMCPは同じhostで解決します。host自身の実行場所から同梱環境を探すため、アプリを移動しても保存済みの開発用パスに依存しません。明示した開発用環境変数は優先します。同梱環境があるのにファイルが欠落していた場合は、別の環境へ黙って切り替えずエラーにします。アプリ外の開発hostは従来の環境変数・保存設定を使えます。

ブラウザの実行ファイルはアプリ内、画像・一時プロファイルは設定済み外部ディスクに置きます。ブラウザのサンドボックス、loopback素材配信の制限、実GPU検査、キャンセルと子プロセス終了検査を維持します。初回起動時のネット取得はありません。

## ビルドと更新

`npm run desktop:bundle` は `scripts/prepare-vrm-bundle.mjs` を実行します。

- Nodeは `.node-version` の公式アーカイブを取得し、公式SHASUMS256と照合。
- ブラウザは固定Playwrightの `browsers.json` と一致するHeadless Shellのみを取得。
- キャッシュは外部SSD上のこのcheckoutの `target/vrm-downloads`。既存キャッシュを使う場合はビルド時に `SUNGEKI_BUNDLE_BROWSER_CACHE` を指定。
- 同梱ファイルのhash・各バージョン・Node取得元をmanifestに記録し、bundle後にも全ファイルを照合。
- Node、ブラウザ、Playwrightはアプリ更新時にまとめて更新・描画回帰を行う。OSのブラウザ更新とは独立する。

## ライセンス

Node公式配布のLICENSE（依存の通知を含む）、Headless Shell付属のLICENSE.headless_shell、Playwright/Playwright Core/Three.jsのライセンス・通知を保持します。フロントエンドの共有VRM依存は既存のTHIRD_PARTY_FRONTENDにも収録されます。素材のライセンスは別途利用者が確認してください。

- Node: https://github.com/nodejs/node/blob/v22.22.3/LICENSE
- Chromium: https://chromium.googlesource.com/chromium/src/+/153.0.8010.12/LICENSE
- Chromium source: https://chromium.googlesource.com/chromium/src/+/153.0.8010.12/
- Playwright: https://github.com/microsoft/playwright/tree/v1.63.0

一般公開用の署名・公証と再配布に必要なソース提供範囲の最終確認は、ローカル同梱動作の検証とは分けます。
