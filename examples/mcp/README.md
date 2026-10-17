# TypeScriptから新MCPで動画を作る

Nodeの標準機能とSungeki生成型だけを使う、ローカルstdio MCPの実行例。起動するのはSungekiのMCPクライアントで、編集・制作・保存は共通hostが行う。

## 準備

リポジトリの固定Node/npmで`npm ci`を済ませ、次を実行する。

```sh
npm run check:toolchain
cargo build --release --locked -p sungeki-host -p sungeki-mcp
node --experimental-strip-types examples/mcp/production.ts capabilities
```

既定の実行ファイルはこのリポジトリの`target/release/sungeki-mcp`。別配置なら`SUNGEKI_MCP_BINARY`に絶対パスを指定する。hostの接続先・保存済み実行環境設定は通常の新MCPと同じ。[開発手順](../../docs/DEVELOPMENT.md)を参照。サンプルがモデルや描画環境を自動導入することはない。

## 制作要求を保存する

`request.json`は新MCPの`build_video`へ送る[StartBuildVideoArgs](../../crates/protocol/src/build_video.rs)そのもの。`recipe`は[BuildRecipe](../../packages/api-client/src/generated.ts)の形式で、構成は[テンプレートガイド](../../templates/README.md)を参照する。

次のTypeScriptで、自分の検査済みrecipeから要求を一度だけ保存できる。`recipe.json`には素材の検査結果、声、立ち絵、背景、台本、クレジット等を含める。`tests/fixtures/recipes/build-dialogue.json`は構造例であり、架空の素材を含むのでそのまま実制作には使えない。

```ts
import { randomUUID } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import type { BuildRecipe, StartBuildVideoArgs } from '../../packages/api-client/src/generated.ts';

const recipe: BuildRecipe = JSON.parse(await readFile('recipe.json', 'utf8'));
const request: StartBuildVideoArgs = {
  job_id: randomUUID(),
  recipe,
  project_path: '/absolute/new-folder/作品の固有名.vvm.json',
  base_url: 'http://127.0.0.1:55021/',
  engine_version: '実エンジンから取得した版',
};
await writeFile('request.json', JSON.stringify(request, null, 2), { flag: 'wx' });
```

この断片をファイルにする場合は`examples/mcp/`へ置くと型importの相対パスが一致する。実行時のJSON検証はhostが行う。TypeScriptの型注釈だけでJSONの正当性は保証されない。素材を固定し、未使用の保存先を選ぶ。SBV2/Qwen/Irodoriは`managed_profiles`へcharacter ID→登録設定IDを指定する。

Markdownを使う場合は[client.ts](client.ts)の`call('parse_markdown_script', {source, roles})`で共通解析し、返った各場面に背景・間隔・演出を明示してrecipeへ入れる。解析だけでは素材設定や合成は行わない。

## 開始・追跡・取消

```sh
node --experimental-strip-types examples/mcp/production.ts start /absolute/request.json
node --experimental-strip-types examples/mcp/production.ts watch JOB_ID
node --experimental-strip-types examples/mcp/production.ts status JOB_ID
node --experimental-strip-types examples/mcp/production.ts cancel JOB_ID
```

- `start`は開始受付後に戻り、job/session IDを表示する。`watch`は30秒以内の有限待機を繰り返し、同じjobを追跡する。`status`は現在の状態を一回取得する。
- 応答が時間切れになっても、新しいIDで作り直さず、保存済み要求のjob IDを`status`/`watch`で確認する。同じ要求ファイル・同じIDの再送は同じ制作を指す。内容を変更した新規制作では新ID・新保存先を使う。
- クライアント終了は制作取消ではない。`cancel`の応答が`cancelling`なら`watch`で終了を確認する。
- `failed`・`cancelled`・`interrupted`は終了コード1、引数の誤りは2。それ以外の正常な状態取得は0。`start`の終了コード0だけで動画完成とは判断しない。
- 成功後に作品・MP4のパスと確認レポートの集計を表示する。`succeeded`でも`not_run`や`complete:false`が残りうる。全編の映像・音声確認は別途必要。

全セリフを保存音声から引き継ぐ場合は、`read_build_reuse_source`の候補を確認し、要求に`reuse_from: {parent_job_id, speech: {"0": 0}}`のように新セリフ番号→元セリフ番号を指定する。双方0始まり。話者・字幕・読み・原本と音声設定は開始時に再検査される。一部しか割り当てなければ残りは新規合成する。

VOICEVOXを新規合成するAI制作では、所有する管理セッションで`voicevox_ctl.sh start`→`wait`→制作→動画出力直後の`stop`→`status`のSTOPPED確認を完結する。失敗・取消でも停止する。他の制作が使っているエンジンを停止しない。このCLIは管理セッションの所有権を判断しないため、勝手にエンジンを起動・停止しない。管理型の他エンジンはhostの設定に従う。

## 検査

```sh
node node_modules/typescript/bin/tsc -p examples/mcp/tsconfig.json
node --experimental-strip-types --test examples/mcp/client.test.ts
```


## 制作後のシーン編集

[scene-edit.ts](scene-edit.ts)は、生成した作品をシーン単位で移動する例。session/scene IDは`list_sessions`と`get_project(view:"scenes")`で対象作品から取得する。前面の作品を推測して選ばない。

```ts
import { writeFile, readFile } from 'node:fs/promises';
import { McpClient, McpToolError } from './client.ts';
import { planSceneMove, applySceneMove, sceneMoveUndo } from './scene-edit.ts';

const client = new McpClient('/absolute/target/release/sungeki-mcp');
try {
  await client.initialize();
  const plan = await planSceneMove(client, '対象session ID', '対象scene ID', 12_000_000);
  await writeFile('scene-plan.json', JSON.stringify(plan, null, 2), {flag:'wx'});
  // plan.previewsの全ページを確認する。ここまで作品は未変更。
  // 適用する段階で、保存した同じ要求を使う。
  const checked = JSON.parse(await readFile('scene-plan.json', 'utf8'));
  const applied = await applySceneMove(client, checked);
  await writeFile('scene-edit-result.json', JSON.stringify(applied, null, 2), {flag:'wx'});
  // 必要ならこの編集だけをUndo。先に要求を保存して再送可能にする。
  const undo = sceneMoveUndo(applied);
  await writeFile('scene-undo.json', JSON.stringify(undo, null, 2), {flag:'wx'});
  // await client.call('undo', undo);
} catch (error) {
  if (error instanceof McpToolError && error.code === 'CONFLICT') {
    console.error('別の編集が入りました。作品を読み直して差分を確認してください。');
  } else throw error;
} finally { await client.close(); }
```

`startUs`はシーンの新しい絶対開始時刻（整数マイクロ秒）。所属クリップは共通editorで同じ差分だけ移動する。差分は全ページ取得し、ページの途中で作品が変わっても拒否する。適用は計画時のrevisionを固定するため、後から入った人間の編集を上書きしない。衝突時にrevisionを自動更新して再送しない。

適用応答が失われたら保存済みの同じoperation IDを再送する。Undoも記録したrevisionと対象operation IDを指定し、他の編集を誤って戻さない。編集後の文書保存は`save_project`で明示する。上記の例は自動保存しない。シーン名を変えた場合などは既存の動画・確認レポートを新状態の出力と取り違えない。
