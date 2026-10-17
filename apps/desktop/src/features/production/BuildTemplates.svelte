<script lang="ts">
  import type { BuildRecipe } from '@sungeki/api-client';
  import type { BuildJobsController } from './jobs.svelte';
  import type { TemplateLibrary } from './template-library.svelte';
  import TemplateRun from './TemplateRun.svelte';
  import { locale } from '../../shared/i18n/locale.svelte';
  let { library, currentRecipe, jobs, disabled, onbusy, loadOnly = false }: { library: TemplateLibrary; currentRecipe: BuildRecipe | null; jobs: BuildJobsController; disabled: boolean; onbusy: (value: boolean) => void; loadOnly?: boolean } = $props();
  const t = (ja: string, en: string) => locale.current === 'en' ? en : ja;
  let name = $state(''), running = $state(false), confirmUpdate = $state(false);
  const locked = $derived(disabled || running || library.busy || !!library.pending);
</script>
<details open={loadOnly}>
  <summary>{loadOnly ? t('保存テンプレートから制作する', 'Create from a saved template') : t('制作テンプレートを保存・読み込む', 'Save or load production templates')}</summary>
  <p>{loadOnly ? t('作品を開かずに、保存した話者・素材・台本を使って新しい動画を作れます。', 'Create a new video with saved speakers, media and script without opening a project.') : t('上の一括制作設定を保存して再利用できます。音声モデルや素材本体、出力先はコピーしません。', 'Save the production settings above for reuse. Models, media files and output destinations are not copied.')}</p>
  <fieldset disabled={locked}>
    {#if !loadOnly}
    <label>{t('テンプレート名', 'Template name')}<input bind:value={name} maxlength="1024" /></label>
    <button type="button" disabled={!currentRecipe || !name.trim()} onclick={() => library.save(currentRecipe!, name)}>{t('上の設定を新しく保存', 'Save the settings above as new')}</button>
    {#if !currentRecipe}<p>{t('保存するには、上の作品名・人物・背景・台本などを入力してください。読み込みは入力前でも使えます。', 'Complete the title, characters, backgrounds and script above to save. Loading is available before filling the form.')}</p>{/if}
    {/if}
    <button type="button" onclick={() => library.request({ action: 'list', after_id: null, limit: 20 })}>{t('保存済みを表示', 'Show saved templates')}</button>
    {#each library.items as item}<button type="button" class:selected={item.id === library.selected?.id} onclick={() => { confirmUpdate = false; void library.request({ action: 'get', id: item.id }); }}>{item.name}</button>{/each}
    {#if library.next}<button type="button" onclick={() => library.request({ action: 'list', after_id: library.next, limit: 20 })}>{t('続きを表示', 'Load more')}</button>{/if}
    {#if library.selected?.preset && library.recipe}
      <h3>{library.selected.preset.name}</h3>
      <p>{t('保存版', 'Saved revision')} {library.selected.revision} · {library.recipe.roles.map(r => r.character.name).join(' / ')} · {library.recipe.scenes.length} {t('シーン', 'scenes')}</p>
      {#if !loadOnly}{#if confirmUpdate}
        <p>{t('上の一括制作設定とテンプレート名で保存版を置き換えます。下の文章編集は含みません。既存作品は変わりません。', 'Replace the saved version with the production settings and template name above, excluding text edits below. Existing projects stay unchanged.')}</p>
        <button type="button" disabled={!currentRecipe || !name.trim()} onclick={() => { confirmUpdate = false; void library.save(currentRecipe!, name, true); }}>{t('更新を確定', 'Confirm update')}</button>
        <button type="button" onclick={() => confirmUpdate = false}>{t('取消', 'Cancel')}</button>
      {:else}<button type="button" disabled={!currentRecipe || !name.trim()} onclick={() => confirmUpdate = true}>{t('上の設定で保存版を更新', 'Update saved version from settings above')}</button>{/if}{/if}
    {/if}
  </fieldset>
  {#if library.pending}<button type="button" disabled={library.busy || disabled || running} onclick={() => library.request(library.pending!)}>{t('同じ保存操作を再確認', 'Retry the same save')}</button>{/if}
  {#if library.error}<p role="alert">{library.error}</p>{/if}
  {#if library.recipe && library.selected}
    {#key `${library.selected.id}:${library.selected.revision}`}
      <TemplateRun bind:recipe={library.recipe} {jobs} disabled={locked} onbusy={v => { running = v; onbusy(v); }} />
    {/key}
  {/if}
</details>
<style>details { border:1px solid var(--panel-border); padding:14px; margin:12px 0; } fieldset { border:0; padding:0; } label { display:grid; gap:6px; margin:10px 0; } button { margin:4px; } .selected { outline:2px solid currentColor; }</style>
