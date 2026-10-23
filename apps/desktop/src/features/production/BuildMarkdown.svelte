<script lang="ts">
  import { onDestroy } from 'svelte';
  import { hostApi } from '../../shared/host-api';
  import { locale } from '../../shared/i18n/locale.svelte';
  import { MarkdownInput, markdownAppendSummary } from './markdown-input.svelte';
  import type { SceneInput } from './scenes';
  let { source = $bindable(), scenes = $bindable(), narration, disabled }: {
    source: string; scenes: SceneInput[]; narration: boolean; disabled: boolean;
  } = $props();
  const input = new MarkdownInput(hostApi);
  const t = (ja: string, en: string) => locale.current === 'en' ? en : ja;
  const result = $derived(input.result?.source === source && input.result.narration === narration ? input.result : null);
  const summary = $derived(result ? markdownAppendSummary(scenes, result.scenes) : null);
  onDestroy(() => input.dispose());
  function append() {
    if (disabled || input.busy || !result || !summary?.allowed) return;
    scenes = [...scenes, ...structuredClone($state.snapshot(result.scenes))];
    input.result = null;
  }
</script>
<details>
  <summary>{t('Markdown台本から場面を追加', 'Add scenes from a Markdown script')}</summary>
  <p>{t('## シーン名、- left: セリフ の形式で入力します。二人の場合は right も使えます。読みだけ変える場合は直後の行に半角空白2つと > 読み を指定します。', 'Use ## Scene name and - left: Speech. Dialogue also supports right. For a separate pronunciation, follow the speech with two spaces and > Spoken text.')}</p>
  <p>{t('既存の場面の後ろに追加します。追加後に各場面の背景・間隔・演出を設定してください。解析だけでは作品を変更せず、音声も生成しません。', 'Scenes are appended after the existing ones. Choose their backgrounds, gaps and effects afterward. Parsing does not edit the project or synthesize audio.')}</p>
  <label>{t('Markdown台本（100,000文字・200セリフまで）', 'Markdown script (up to 100,000 characters and 200 speeches)')}
    <textarea rows="7" bind:value={source} disabled={disabled || input.busy} placeholder={locale.current === 'en' ? '## Introduction\n- left: Hello.' : '## 導入\n- left: こんにちは。'}></textarea>
  </label>
  <button type="button" disabled={disabled || input.busy || !source.trim()} onclick={() => input.parse(source, narration)}>{t('台本を確認', 'Check script')}</button>
  {#if input.busy}<p role="status">{t('台本を確認しています…', 'Checking script…')}</p>{/if}
  {#if input.error}<p role="alert">{input.error}</p>{/if}
  {#if result}
    <ol>{#each result.scenes as scene}<li>{scene.name}</li>{/each}</ol>
    {#if summary?.invalidScene}
      <p role="alert">{t('先に台本の書式を修正してください。場面', 'Fix the script format first. Scene')} {summary.invalidScene}</p>
    {:else if summary}
      <p role="status">{t('追加後の合計：', 'Total after adding:')} {summary.scenes} {t('場面・', 'scenes /')} {summary.speeches} {t('セリフ', 'speeches')}</p>
      {#if !summary.allowed}<p role="alert">{t('制作できるのは全体で200場面・200セリフまでです。既存の場面も含めて減らしてください。', 'Production supports at most 200 scenes and 200 speeches in total. Reduce the total, including existing scenes.')}</p>{/if}
    {/if}
    <button type="button" disabled={disabled || input.busy || !summary?.allowed} onclick={append}>{t('確認した場面を末尾に追加', 'Append the checked scenes')}</button>
  {/if}
</details>
<style>details { border:1px solid var(--panel-border); padding:12px; margin:12px 0; } label { display:grid; gap:6px; } textarea { width:100%; } ol { max-height:180px; overflow:auto; } p { overflow-wrap:anywhere; }</style>
