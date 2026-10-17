<script lang="ts">
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { dialogueText as t } from './dialogue-template-messages';
  import type { SessionSnapshot } from '@sungeki/api-client';
  import type { EditorController } from '../editor/controller.svelte';
  import { hostApi } from '../../shared/host-api';
  import { EditPlan } from '../scenes/edit-plan.svelte';
  import EditReview from '../scenes/EditReview.svelte';
  import EditorNotices from '../editor/EditorNotices.svelte';
  let { editor }: { editor: EditorController } = $props();
  let dialog: HTMLDialogElement;
  let observed = $state.raw<SessionSnapshot | null>(null);
  let title = $state(''), start = $state(0), gap = $state(.35);
  let background = $state(''), bgm = $state('');
  let left = $state(''), right = $state(''), leftStanding = $state(''), rightStanding = $state('');
  let selected = $state<string[]>([]);
  const plan = new EditPlan(hostApi);
  let clips = $derived(observed?.project.clips.filter(c => !c.scene_id) ?? []);
  let speeches = $derived(clips.filter(c => c.content.type === 'speech').sort((a,b) => a.start_us-b.start_us || a.id.localeCompare(b.id)));
  let visuals = $derived(clips.filter(c => c.content.type === 'image' || c.content.type === 'psd'));
  let stale = $derived(!!observed && (observed.revision !== editor.snapshot?.revision || observed.session_id !== editor.snapshot?.session_id));
  let disabled = $derived(stale || editor.busy || !!editor.pending || editor.drafting);
  function label(id: string) {
    const c = clips.find(c => c.id === id)?.content;
    return c && 'asset_id' in c ? observed?.project.assets.find(a => a.id === c.asset_id)?.name ?? id : id;
  }
  function refresh() {
    plan.cancel(); observed = editor.snapshot;
    title = observed?.project.title ?? ''; start=0;
    left=observed?.project.characters[0]?.id ?? ''; right=observed?.project.characters[1]?.id ?? '';
    background=''; bgm=''; leftStanding=''; rightStanding=''; selected=speeches.map(c=>c.id);
  }
  async function preview(event: SubmitEvent) {
    event.preventDefault(); if (disabled || !observed) return;
    await plan.prepare(observed,[{type:'apply_dialogue_template',template:{preset_id:'dialogue-wide-v1',title,
      speech_clip_ids:speeches.filter(c=>selected.includes(c.id)).map(c=>c.id),
      participants:[{character_id:left,standing_clip_id:leftStanding},{character_id:right,standing_clip_id:rightStanding}],
      background_clip_id:background,bgm_clip_id:bgm,start_us:Math.round(start*1e6),gap_us:Math.round(gap*1e6),
      scene_id:crypto.randomUUID(),track_ids:Array.from({length:6},()=>crypto.randomUUID()),title_clip_id:crypto.randomUUID(),credits_clip_id:crypto.randomUUID(),title_backdrop_id:crypto.randomUUID(),credits_backdrop_id:crypto.randomUUID(),
    }}]);
  }
  async function confirm() {
    if(disabled || plan.busy || plan.stale(editor.snapshot) || !plan.request || !plan.preview || plan.error) return;
    await editor.request(plan.request); if(!editor.error && !editor.pending){plan.cancel();dialog.close();}
  }
</script>
<button disabled={editor.busy || !!editor.pending || editor.drafting} onclick={()=>{refresh();dialog.showModal();}}>{t("掛け合いの構成を整える")}</button>
<dialog use:containDialogFocus bind:this={dialog} onclose={()=>plan.cancel()} aria-label={t("掛け合いの構成")}>
  <header><h2>{t("掛け合いの構成")}</h2><button onclick={()=>dialog.close()}>{t("閉じる")}</button></header>
  <EditorNotices {editor}/>
  <p>{t("横動画（1920×1080・30fps）用です。先に二人の音声を生成し、背景画像・立ち絵2枚・BGMを追加してください。素材と話者のクレジットも必要です。")}</p>
  <p>{t("タイトル3秒 → 掛け合い → クレジット3.5秒を作ります。背景・立ち絵・字幕の配置を変更し、BGMは全体で繰り返し、−18dB・前後1秒フェードに設定します。立ち絵のポーズと口形設定は維持します。映像フェードはまだ付きません。")}</p>
  <p>{t("シーン未所属のクリップを選んでください。対象外のクリップと重なる時間には配置できません。全変更は一度の「元に戻す」で取り消せます。")}</p>
  {#if stale}<p role="alert">{t("作品が更新されました。最新の状態を読み込んでください。")}</p>{/if}
  <button disabled={editor.busy || !!editor.pending} onclick={refresh}>{t("入力を破棄して最新の状態を読む")}</button>
  {#if observed && plan.request}<EditReview {plan} project={observed.project} stale={stale || plan.stale(editor.snapshot)} {disabled} {confirm}/>
  {:else}<form onsubmit={preview}><fieldset disabled={disabled}>
    <label>{t("タイトル")}<textarea required bind:value={title} rows="2"></textarea></label>
    <label>{t("開始位置（秒）")}<input type="number" required min="0" max="604800" step="0.000001" bind:value={start}/></label>
    <label>{t("発話間隔（秒）")}<input type="number" required min="0" max="604800" step="0.000001" bind:value={gap}/></label>
    <label>{t("左の話者")}<select required bind:value={left}><option value="">{t("選択")}</option>{#each observed?.project.characters ?? [] as c}<option value={c.id}>{c.name}</option>{/each}</select></label>
    <label>{t("左の立ち絵")}<select required bind:value={leftStanding}><option value="">{t("選択")}</option>{#each visuals as c}<option value={c.id}>{label(c.id)}</option>{/each}</select></label>
    <label>{t("右の話者")}<select required bind:value={right}><option value="">{t("選択")}</option>{#each observed?.project.characters ?? [] as c}<option value={c.id}>{c.name}</option>{/each}</select></label>
    <label>{t("右の立ち絵")}<select required bind:value={rightStanding}><option value="">{t("選択")}</option>{#each visuals as c}<option value={c.id}>{label(c.id)}</option>{/each}</select></label>
    <label>{t("背景")}<select required bind:value={background}><option value="">{t("選択")}</option>{#each visuals.filter(c=>c.content.type==='image') as c}<option value={c.id}>{label(c.id)}</option>{/each}</select></label>
    <label>BGM<select required bind:value={bgm}><option value="">{t("選択")}</option>{#each clips.filter(c=>c.content.type==='audio') as c}<option value={c.id}>{label(c.id)}</option>{/each}</select></label>
    <fieldset><legend>{t("セリフ（開始時刻順）")}</legend>{#each speeches as c}{#if c.content.type==='speech'}<label><input type="checkbox" value={c.id} bind:group={selected}/>{c.content.subtitle_text}</label>{/if}{/each}</fieldset>
    <button disabled={!selected.length}>{t("変更内容を確認")}</button>
  </fieldset></form>{/if}
</dialog>
<style>
  dialog { max-width:760px; } header { display:flex; align-items:center; justify-content:space-between; }
  label { display:block; margin:10px 0; } select,textarea { display:block; width:100%; }
  fieldset { border:0; padding:10px 0; } p { font-size:13px; }
</style>
