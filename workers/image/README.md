# 画像描画workerの環境と検証

macOS / Apple Siliconで、PSD解析・字幕・正式フレーム描画に使用する独立Python環境。
共通hostの正式動画、確認フレーム、軽量プレビューへ接続済み。依存環境の導入成功、実動画の出力成功、全機能の受入完了は区別する。

## 再現

### 準備するもの

配布アプリとは別に、同じ版のSungekiソース一式が必要です。`package.json`、`scripts/`、`workers/image/`を含むフォルダを用意します。画像処理環境を作るだけならRustやアプリの再ビルドは不要です。現在の画像準備スクリプトはNodeの標準機能だけを使うため、この手順だけのために`npm ci`を実行する必要もありません。3Dやアプリ開発の準備は別の手順です。

使用するツールはNode 22.22.3、npm 10.9.8、uv 0.10.6です。uvがPython本体と画像ライブラリを外部ディスクへ取得するため、初回はインターネット接続が必要です。OSのPythonを入れ替える必要はありません。Node/npm/uv自体のインストールはこの準備コマンドの対象外です。

### ターミナルで実行する順序

まずターミナルで、取得したSungekiソースの`package.json`があるフォルダへ移動します。下記の2つのパスは例なので、ソースの配置先と接続済み外部ディスクの名前に置き換えてください。保存先には、このアプリ専用のフォルダを指定します。

```sh
cd "/ソースを置いた場所/sungeki"
npm run check:toolchain
uv --version
```

固定環境の確認が成功し、uvが`0.10.6`と表示されたら、同じターミナルで続けます。準備コマンドもuvの存在と版を検査し、不足・版違いでは保存先フォルダを作る前に対処方法を表示して停止します。`command not found`の場合は該当ツールの導入・PATHを確認し、版が違う場合は指定版へ切り替えてから進めます。

```sh
export SUNGEKI_RUNTIME_ROOT="/Volumes/動画用SSD/Sungeki/runtime"
npm run setup:image
npm run probe:image
```

setupは指定した専用フォルダを必要に応じて作成します。アプリの設定画面で保存先を登録する時点では、そのフォルダが存在している必要があります。probeが成功すると`image/probes/`の固有フォルダに`report.json`と検証画像ができます。これは画像処理単体の確認です。アプリからの動画出力は、次の登録後に別途確認します。

### アプリへ登録するもの

| 設定画面の項目 | 上の例を使った場合 |
|---|---|
| 外部ディスクの保存フォルダ | `/Volumes/動画用SSD/Sungeki/runtime` |
| Pythonの環境フォルダ | `/Volumes/動画用SSD/Sungeki/runtime/image/venv` |
| 画像処理プログラム | 配布アプリでは空欄にして同梱版を使う |

「設定」→「動画制作の実行環境」で登録します。FFmpeg、FFprobe、字幕フォントも別途指定し、短い文字・図形の作品で「正式な画像を確認」→「動画を書き出す」を確認します。Pythonの準備だけでは、音声エンジンや3D環境は導入されません。全体の順序は[READMEの初回導入手順](../../README.md#初めて使うときの順序macosのローカル検証版)を参照してください。

### 固定版と保存先の詳細

Python 3.12.12、Pillow 12.3.0、psd-tools 1.19.0を固定し、推移依存と配布物ハッシュはuv.lockで管理する。
Pythonは最新版の宣言ではなく、導入済みuvの配布カタログで取得できる再現用pin。
Pillow/psd-toolsのPython >=3.10条件を満たす。合成用extraはまだ導入しない。

例のディスク名は実際の接続済み外部ディスクに置き換える。保存先指定は必須で、アプリの保存済み設定はこの準備コマンドから自動では読み込まない。
保存先は指定したSungeki runtime下 `image/`。Python本体・venv・uv-cache・tmp・検証成果物を置く。
`SUNGEKI_RUNTIME_ROOT`はVRMと同様に、マウント済み外部ディスク内の専用フォルダを指定する。別のディスク名も指定可能。未接続やディスク外を指すリンクは拒否し、内蔵ディスクへ自動的に切り替えない。別名ディスクへの一式導入の実機確認は未完了。
既存環境を使わず導入を確認する場合は、両コマンドへ同じ未使用のSSD上の `SUNGEKI_RUNTIME_ROOT` を指定する。新しい `image/venv/bin/python` をアプリの実行環境設定へ登録するまで、通常の描画環境は切り替わらない。
GUIは「設定」→「動画制作の実行環境」→「保存済み設定を読み込む」から「Pythonの環境フォルダを選ぶ」で `image/venv` を選び、「設定を保存」する。`bin/python`をファイル選択するとmacOSがリンク先の基底Pythonへ解決し、導入したライブラリを使えなくなるため、フォルダから環境内のパスを指定する。既知の実行ファイルの絶対パスを直接入力する方法も維持する。保存成功は依存導入や実際の描画成功を意味しない。
setupはlocked sync、probeはlocked/offline実行。通常操作ではlockを更新しない。
依存を意図的に変更するときだけ `node scripts/image-runtime.mjs lock` を使い、実機probeを再実行する。
uvやPythonをユーザーのbinへ登録せず、OSのPython選択も変更しない。

probeはmacOSのヒラギノ角ゴシックW3を参照し、日本語2行を透過PNGへ描画する。
一層のPSD fixtureを自作し、層名・RGBA画素の往復、不正PSDの拒否を確認する。
出力はUUID別のディレクトリとreport.json。フォントは複製せず、利用パスとSHA-256を記録する。
これはPSD一般の合成互換性、任意字幕の折返し、GUI/MCP編集成功を保証する検証ではない。

仕様確認: [uv環境変数](https://docs.astral.sh/uv/reference/environment/)、
[locked sync](https://docs.astral.sh/uv/concepts/projects/sync/)、
[Pillow](https://pypi.org/project/Pillow/)、[psd-tools](https://pypi.org/project/psd-tools/)。

## フレーム描画と同梱

`render_frame.py` はRustのScenePlanが評価した一フレームをPNGへ描画する。
日本語Text/Speech字幕、矩形/楕円、PNG/JPEG/WebP画像、PSDポーズ、検査済み動画フレーム、背面ぼかし、重なり順・透明度に対応。
画像は指定された固定原本のSHA-256を再検証し、EXIF向き補正後にTransformの寸法へ描く。
preview用縮小画像への代替は行わない。現在のImageモデルとGUIに合わせ拡縮は縦横独立。
アニメーション画像、未知のprimitive、原本hash不一致、過大な寸法等は失敗にする。

フォントの絶対パスとSHA-256を明示し、検証したbytesをPillowへ渡す。
文字の行送りはfont_size×1.3。左/中央/右寄せ、任意の折返し、縁取りに対応し、明示改行と保存された本文を保持する。折返しは字幅・禁則・英数字/カタカナのまとまりを考慮するが、すべての日本語の意味単位を保証するものではない。
文字がTransformの枠またはcanvasをはみ出す場合はTEXT_OVERFLOWとclip ID・実際の描画bboxを返す。
文字を短縮したり枠に合わせて縮めたりせず、作品の編集判断へ返す。
画像レイヤー/図形のcanvas外クリップと、Textのはみ出し診断は別の規則。

製品ではmediaから所有プロセスとして起動し、hostが固定した素材だけを渡す。`render_batch.py` は同じ描画処理で最大32フレームを処理する。
hostのjob/cancel/成果物登録・正式preview_frame/review・MP4合成へ接続済み。
出力先は呼出側の新規staging内とし、既存PNGは上書きしない。失敗した途中ファイルを完成品として登録しない。
現段階のSVG GUIとPillowの字形・ベースラインの完全一致は未検証。

macOSアプリにはworkerのPythonソースと依存定義/lockを同梱し、Python本体とライブラリは含めない。同梱hostはworkerパスが未設定の場合に同梱版を使う。明示したパスの不備から黙って切り替えない。Python・FFmpeg・フォント等は別途設定が必要で、ソース同梱だけを単独配布完成とは扱わない。起動時に同梱ソースへbytecode cacheを書き込まない。

テストは設定済みSSD image/venvのPythonで実行する:

```sh
python -m unittest discover -s workers/image -p 'test_render_frame.py' -v
```

macOSの指定フォントが必須。フォントの変更に伴うレイアウト差を黙って許容する代替フォントは使わない。

## PSD静止ポーズの開発用書き出し

`psd_pose.py --source ABS_PSD --sha256 HASH [--pose PRESET_JSON] [--output NEW_PNG]`。
出力指定がなければレイヤー一覧、あれば埋込previewを使わない全解像度RGBA合成とmanifestを返す。
`test_psd_pose.py`にfixtureの選択・透明度・不正入力・上書き拒否試験を含む。
編集可能なPSD素材、GUI選択、時刻に応じた口形、正式描画へ接続済み。8-bit RGB、可視pixel/groupレイヤー等の制約があり、任意のPSD効果の再現を保証しない。口形の時刻は共通ScenePlanから渡し、このCLIが発話を生成するわけではない。

## 外部画像環境のライセンス表示

画像処理はPillowを通じてFreeType Projectを利用します。FreeTypeの原著作者はDavid Turner、Robert Wilhelm、Werner Lembergです。FreeTypeについてはFTLを選択し、その著作権・許諾・免責原文をPillowの配布通知とともに保持しています。[FreeTypeのライセンス案内](https://freetype.org/license.html)。

固定した画像環境の5パッケージとCPython本体の通知原文は[外部画像環境の通知集](../../licenses/external-image/THIRD_PARTY_IMAGE_RUNTIME.txt)、原文hashと取得範囲は[manifest](../../licenses/external-image/manifest.json)を参照してください。PillowはMIT-CMUに加えて同梱ライブラリの通知を含み、NumPyにも複数の通知があります。本体MITへの置換は行いません。

この通知集は別途導入された開発環境から取得したもので、これらの実行環境が.appに含まれることを意味しません。CPython配布ビルド内の追加ライブラリ等の確認は残っています。通知に記載されたライブラリが、どのOSの配布物にも必ず含まれるとは限りません。
