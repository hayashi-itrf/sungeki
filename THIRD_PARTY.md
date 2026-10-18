# 第三者の権利と素材の扱い

ルートの[MIT License](LICENSE)は、このプロジェクトの独自コードに適用します。第三者のライブラリ、名称、キャラクター、モデル、モーション、音声、画像、フォントをMITで再許諾するものではありません。本プロジェクトはVOICEVOXや各キャラクターの公式配布物ではありません。

## ソースに含まれるもの

| 対象 | 扱い |
|---|---|
| `licenses/frontend/` | Svelte、Tauri API、Three.js、@pixiv/three-vrm関連の固定版の通知原文とmanifest。各原文の著作権表示・許諾条件を保持する。 |
| `licenses/native-supplements/` | Rust依存について上流の固定版から補完した原文。MIT/Apache/MPL等の本文を本体MITで上書きしない。 |
| `licenses/external-image/` | 開発用の比較基準として残すPython画像処理環境の通知。環境バイナリの同梱許可を意味しない。 |
| `apps/desktop/src-tauri/icons/` | プロジェクト内の`source.svg`から生成したアイコン。原本・派生物の対応は開発時に確認。 |
| `tests/fixtures/build-source.png`、`crates/review/tests/data/transparent.png` | 単色の人工テスト画像。 |
| `tests/fixtures/irodori-float.wav` | 5個の数学的サンプル値のみの人工WAV。人物の収録音声やTTSモデル出力ではない。 |

人工媒体は`python3 scripts/check-synthetic-fixtures.py`で内容を確認できます（Pillowが必要）。ソース内のJSON例にキャラクター名やモデル名があっても、その原本や利用権を提供するものではありません。

## 別途用意するもの

FFmpeg/FFprobe、音声合成エンジン・モデル、VRM/VRMA/PSD、キャラクター素材、字幕フォントは各配布元の条件を確認して用意します。エンジンのコード、モデル、生成音声、キャラクターの条件はそれぞれ別です。クレジット要件も使用素材ごとに確認してください。利用者の素材・作品・エンジン設定を公開ソースへ含めないでください。

## ビルド済みアプリの配布は別確認

ソース公開の確認だけで.app/ZIPの配布条件が満たされるわけではありません。実際の同梱物、第三者通知、必要な対象ソースの入手方法、署名・公証、別環境での動作確認を配布物ごとに確認します。

MITの本文と著作権表示を保持する条件は[MIT原文](https://choosealicense.com/licenses/mit/)を参照してください。MPL対象の実行形式を配布する場合の対象ソース入手案内は[MPL 2.0第3.2条](https://www.mozilla.org/en-US/MPL/2.0/)と[Mozilla FAQ](https://www.mozilla.org/en-US/MPL/2.0/FAQ/)を参照してください。本体全体をMPLへ変更するという意味ではありません。

## AivisSpeech / COEIROINK連携（2026-10-18確認）

今回追加した連携は、利用者が別途導入するエンジンを別プロセスで起動し、ローカルHTTP APIへ接続します。この変更でエンジン本体、モデル、キャラクター素材、生成した試験音声・動画をソースやアプリへ同梱していません。新たなライブラリ依存の追加もありません。

- AivisSpeech Engine 1.2.0は[LGPL v3のライセンス本文](https://github.com/Aivis-Project/AivisSpeech-Engine/blob/1.2.0/LICENSE)を持ちます。エンジンのライセンスと音声モデルの条件は別です。[公式規約](https://hub.aivis-project.com/terms-of-service)と選択したモデルのライセンス・追加条件を確認してください。
- COEIROINKは[ソフトウェアの利用規約](https://coeiroink.com/terms)と各音声ライブラリの規約を確認してください。生成音声を利用する作品には所定のクレジットが必要です。[つくよみちゃん](https://coeiroink.com/character/audio-character/tsukuyomi-chan)の例は `COEIROINK:つくよみちゃん` です。
- GUIで取得・表示する話者の利用条件は確認補助です。作品へのクレジット記載や用途ごとの条件遵守を自動的に完了する機能ではありません。音声の利用許諾とキャラクター画像の利用許諾も別です。
- SungekiはVOICEVOX、Aivis Project、COEIROINKおよび話者権利者の公式製品ではなく、提携・推奨を示しません。

将来エンジンやモデルを同梱する場合は、再配布条件、必要な通知・対象ソースの提供を別途確認します。本体MITでこれらを再許諾しません。

## 内蔵の画像描画

PSDの解析はag-psd（MIT）、色変換はLittle CMS（MIT）、文字描画はFreeTypeを使用します。FreeTypeはFTLを選択し、原文と付属ドライバーの通知をアプリのnative-noticesに収録します。

Portions of this software are copyright © The FreeType Project (www.freetype.org). All rights reserved.

これらはアプリに組み込まれ、利用者がPython・uv・Pillowを導入する必要はありません。音声エンジン固有の実行環境は別です。
