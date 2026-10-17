# 制作テンプレート

共通hostの`build_video`で利用する制作構成です。素材や音声モデルは含みません。

| テンプレートID | 構成 |
|---|---|
| dialogue-wide-v1 | 横画面の二人の掛け合い |
| narration-wide-v1 | 横画面の一人ナレーション |
| narration-portrait-v1 | 縦画面の一人ナレーション |
| foreword-wide-v1 / foreword-portrait-v1 | 前書きの構成例 |

テンプレートを元に、検査済みの素材、キャラクタープリセットID、話者、読み、音声設定、背景、台本、クレジットを指定してください。素材の原本と利用条件は利用者が用意します。サンプルJSON内のIDを、自分の環境で登録したIDへ置き換えてください。

引数の正本は[StartBuildVideoArgs](../crates/protocol/src/build_video.rs)、型は[BuildRecipe](../packages/api-client/src/generated.ts)です。[TypeScriptの制作例](../examples/mcp/README.md)から利用できます。`tests/fixtures/recipes/`は構造試験用で、実在する素材を保証しません。

制作後は映像と音声を確認してください。字幕切れ、人物の向き、素材欠落、セリフの切れ、クレジット、出力尺を確認し、原本を変更・破棄する前に作品と素材を保存します。処理中断時は所有する音声エンジンの管理セッションを終了し、他の制作が使うエンジンを停止しないでください。
