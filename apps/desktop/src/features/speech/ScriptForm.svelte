<script lang="ts">
  import { extraText } from '../../shared/i18n/extra-ui';
  import { scriptText as t } from './script-messages';
  import { isBlankScriptLine } from './script-input';
  import type { Command } from '@sungeki/api-client';
  import type { EditorController } from '../editor/controller.svelte';
  let { editor, submitting }: { editor: EditorController; submitting: (id: string) => void } = $props();
  const project = (() => editor.snapshot!.project)();
  let characterId = $state(project.characters[0]?.id ?? '');
  let text = $state('');
  let assignments = $state<{ subtitle_text: string; character_id: string }[] | null>(null);
  function assignSpeakers() {
    if (limitExceeded || !count || !characterId) return;
    assignments = inputLines.map(subtitle_text => ({ subtitle_text, character_id: characterId }));
  }
  let sceneId = $state('');
  let start = $state((() => editor.time)());
  let duration = $state(5), gap = $state(0.35);
  const inputLines = $derived(text.split(/\r?\n/));
  const lineCount = $derived(text ? inputLines.length : 0);
  const characterCount = $derived(Array.from(text).length);
  const count = $derived(inputLines.filter(line => !isBlankScriptLine(line)).length);
  const limitExceeded = $derived(lineCount > 200 || characterCount > 100000);
  async function add(event: SubmitEvent) {
    event.preventDefault();
    if (editor.busy || editor.pending || limitExceeded) return;
    const values = [start, duration, gap].map(v => Math.round(v * 1e6));
    if (!values.every(v => Number.isSafeInteger(v) && v >= 0 && v <= 604800e6) || values[1] === 0) {
      editor.error = t("開始・仮の長さ・間隔を確認してください。"); return;
    }
    const lines = inputLines.map((subtitle_text, index) => ({ clip_id: crypto.randomUUID(), subtitle_text, spoken_text: null,
      character_id: assignments?.[index].character_id ?? null }));
    const first = lines.find(line => !isBlankScriptLine(line.subtitle_text));
    if (!first) { editor.error = t("セリフを入力してください。"); return; }
    const trackId = crypto.randomUUID();
    const commands: Command[] = [
      { type: 'add_track', track: { id: trackId, name: extraText("台本のナレーション"), kind: 'audio', visible: true, muted: false } },
      { type: 'append_speech_script', script: { lines, character_id: characterId, track_id: trackId,
        scene_id: sceneId || null, start_us: values[0], provisional_duration_us: values[1], gap_us: values[2] } },
    ];
    submitting(first.clip_id);
    await editor.apply(commands);
  }
</script>
<form onsubmit={add} oninput={() => { editor.drafting = true; }}>
  <p>{t("1行を1つのセリフとして、新しい音声トラックに追加します。空行は飛ばします。話者名やMarkdownの記号も本文として扱います。")}</p>
  {#if !project.characters.length}<p role="status">{t("先に「＋ キャラクター」から声を登録してください。")}</p>{/if}
  <fieldset disabled={editor.busy || !!editor.pending || !project.characters.length}>
    <label>{t("標準の話者")}<select bind:value={characterId} disabled={!!assignments}>{#each project.characters as character}<option value={character.id}>{character.name}</option>{/each}</select></label>
    <label>{t("台本（空行を含め200行・100,000文字まで）")}<textarea rows="8" disabled={!!assignments} aria-describedby="script-input-count script-input-limit" aria-invalid={limitExceeded} bind:value={text} required placeholder={t("こんにちは。\n次のセリフです。")}></textarea></label>
    <p id="script-input-count" role="status">{t("現在 {lines} / 200行（空行を含む）・{characters} / 100,000文字（改行を含む）", { lines: lineCount.toLocaleString(), characters: characterCount.toLocaleString() })}</p>
    <div id="script-input-limit" role="alert">
      {#if limitExceeded}
        <p>{t("入力内容は切り詰めずに保持しています。次の上限を超えているため追加できません。")}</p>
        {#if lineCount > 200}<p>{t("行数を{count}行以上減らし、200行以内にしてください。", { count: (lineCount - 200).toLocaleString() })}</p>{/if}
        {#if characterCount > 100000}<p>{t("文字数を{count}文字以上減らし、100,000文字以内にしてください。", { count: (characterCount - 100000).toLocaleString() })}</p>{/if}
      {/if}
    </div>
    {#if assignments}
      <p>{t("行ごとの話者を確認してください。台本を修正する場合は、下のボタンで割当を解除して戻ります。")}</p>
      <button type="button" onclick={() => { assignments = null; }}>{t("台本を書き直す（話者の割当を解除）")}</button>
      <div class="speakers">
        {#each assignments as line, index}
          {#if !isBlankScriptLine(line.subtitle_text)}
            <label>{t("{line}行目：", { line: index + 1 })}{line.subtitle_text}
              <select aria-label={t("{line}行目の話者", { line: index + 1 })} bind:value={line.character_id}>
                {#each project.characters as character}<option value={character.id}>{character.name}</option>{/each}
              </select>
            </label>
          {/if}
        {/each}
      </div>
    {:else}
      <button type="button" disabled={!count || limitExceeded} onclick={assignSpeakers}>{t("行ごとに話者を割り当てる")}</button>
      <p>{t("割り当てない場合は、すべて標準の話者で追加します。")}</p>
    {/if}
    <div class="placement">
      <label>{t("開始（秒）")}<input type="number" min="0" max="604800" step="0.000001" required bind:value={start} /></label>
      <label>{t("1行の仮の長さ（秒）")}<input type="number" min="0.000001" max="604800" step="0.000001" required bind:value={duration} /></label>
      <label>{t("セリフ間の間隔（秒）")}<input type="number" min="0" max="604800" step="0.000001" required bind:value={gap} /></label>
    </div>
    <label>{t("所属シーン")}<select bind:value={sceneId}><option value="">{t("シーンなし")}</option>{#each project.scenes as scene}<option value={scene.id}>{scene.name}</option>{/each}</select></label>
    <p>{t("音声はまだ生成しません。仮の長さで配置し、既存のクリップは動かしません。シーンを選ぶ場合は、その区間内に収めてください。")}</p>
    <p>{t("「音声合成」で生成後、「音声の長さで並べる」で配置を調整できます。追加全体は「元に戻す」1回で取り消せます。")}</p>
    <button type="submit" disabled={!count || limitExceeded}>{t("{count}件のセリフを追加", { count })}</button>
  </fieldset>
</form>
<style>
  fieldset { border:0; padding:0; }
  label { display:flex; flex-direction:column; gap:5px; margin:12px 0; }
  textarea, input, select { width:100%; }
  textarea { resize:vertical; }
  .speakers { max-height:320px; overflow:auto; padding:10px; border:1px solid var(--border); }
  .speakers label { overflow-wrap:anywhere; }
  .placement { display:grid; grid-template-columns:repeat(3,1fr); gap:12px; }
  p { font-size:12px; }
</style>
