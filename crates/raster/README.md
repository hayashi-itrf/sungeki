# Native rasterization

`ScenePlan`で確定した画像・PSD・文字・図形・ぼかしをRGBA/PNGへ描画します。Projectの編集や時刻計算は行いません。Pythonや外部画像workerを起動せず、FFmpegによる動画・音声の符号化は`render`側で行います。

原本のサイズとSHA256を検査し、symlinkを拒否します。job内の画像キャッシュは256MiBまで。描画結果のlayout versionは10で、過去の確認画像は新しい結果として扱いません。

- PSD: 8-bit RGB、レイヤー選択、通常・乗算・通過、透明度、ICC→sRGB。埋め込みプレビューは使用しません。
- 可視マスク・クリッピング・レイヤー効果・調整・テキスト・スマートオブジェクトなど未対応のPSD機能はエラーを返します。
- 文字: FreeTypeによるフォント描画、縁取り、改行・禁則・配置と溢れ検出。全言語の組版・全フォントでの画素一致を保証しません。
- Little CMS / FreeTypeは静的リンクします。FreeTypeはFTLを選択し、アプリ同梱通知に原文とクレジットを収録します。

`cargo test --release -p sungeki-raster`で人工fixtureを確認できます。`examples/psd_pose.rs`は手元のPSDとpose JSONの比較検証用です。第三者の立ち絵やフォントは含みません。
