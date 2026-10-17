<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
  import { save } from '@tauri-apps/plugin-dialog';
  import type { BuildRecipe } from '@sungeki/api-client';
  import type { BuildJobsController } from './jobs.svelte';
  import type { BuildProfile } from './voice-settings';
  import { buildProfiles, buildConnection, savedVoicevoxConnection } from './voice-settings';
  import BuildReusePicker from './BuildReusePicker.svelte';
  import { BuildReuseSelection } from './reuse.svelte';
  import BuildProfilePicker from './BuildProfilePicker.svelte';
  import { hostApi } from '../../shared/host-api';
  import { locale } from '../../shared/i18n/locale.svelte';
  import { voicevoxUrl, rememberVoicevoxUrl } from '../speech/connection';
  let { recipe = $bindable(), jobs, disabled, onbusy }: { recipe: BuildRecipe; jobs: BuildJobsController; disabled: boolean; onbusy: (value: boolean) => void } = $props();
  const t = (ja: string, en: string) => locale.current === 'en' ? en : ja;
  const people = $state<{ character: string; profile?: BuildProfile }[]>(untrack(() => recipe.roles.map(r => ({ character: r.character.id }))));
  let url = $state(voicevoxUrl()), busy = $state(false), error = $state('');
  const reuse = new BuildReuseSelection(hostApi);
  const locked = $derived(disabled || busy || reuse.busy);
  const savedConnection = $derived(savedVoicevoxConnection(recipe, reuse.source
    ? { parent_job_id: reuse.source.job_id, speech: reuse.choices } : undefined, reuse.source));
  let alive = true;
  onDestroy(() => { alive = false; reuse.dispose(); onbusy(false); });
  async function start() {
    if (locked) return;
    busy = true; onbusy(true); error = '';
    try {
      const captured = JSON.parse(JSON.stringify(recipe)) as BuildRecipe;
      const reuse_from = reuse.request(captured);
      const reuseSource = reuse.source;
      const profiles = buildProfiles(captured, people);
      const path = await save({ title: locale.text('productionSaveTitle'), defaultPath: `${captured.title.replace(/[\\/:*?"<>|]/g, '_')}.vvm.json`, filters: [{ name: locale.text('productionProject'), extensions: ['json'] }] });
      if (!path || !alive) return;
      const connection = await buildConnection(hostApi, captured, url, reuse_from, reuseSource);
      if (!alive) return;
      if (connection.base_url) rememberVoicevoxUrl(connection.base_url);
      await jobs.start({ recipe: captured, managed_profiles: profiles, project_path: path, ...connection, reuse_from });
    } catch (e) { if (alive) error = e && typeof e === 'object' && 'message' in e ? String(e.message) : String(e); }
    finally { busy = false; onbusy(false); }
  }
</script>
<fieldset disabled={locked}>
  <legend>{t('テンプレートから新しい動画を作る', 'Create a new video from this template')}</legend>
  <p>{t('保存された声・立ち絵・背景・図版・BGM・演出を使用します。下の文章変更は今回の制作だけに使い、保存元や編集中の作品は変わりません。', 'Uses saved voices, standing, backgrounds, diagrams, BGM and effects. Text edits below affect this build only; the saved template and open project stay unchanged.')}</p>
  <label>{t('新しい作品名', 'New project title')}<input bind:value={recipe.title} /></label>
  {#each recipe.scenes as scene}
    <h4>{scene.name}</h4>
    {#each scene.lines as line}
      <label>{recipe.roles.find(r => r.role === line.role)?.character.name ?? line.role}<textarea bind:value={line.subtitle_text} rows="2"></textarea></label>
      <label>{t('読み上げ専用の文章（空欄なら字幕を読む）', 'Spoken text (blank uses subtitles)')}<input value={line.spoken_text ?? ''} oninput={e => line.spoken_text = e.currentTarget.value || null} /></label>
    {/each}
  {/each}
  {#each recipe.roles as role, i}
    <p>{role.character.name} · {role.character.credit}</p>
    {#if role.character.voice.type !== 'voicevox'}<BuildProfilePicker engine={role.character.voice.type} person={people[i]!} disabled={disabled || busy} />{/if}
  {/each}
  {#if !savedConnection && recipe.roles.some(r => r.character.voice.type === 'voicevox')}<label>VOICEVOX URL<input type="url" bind:value={url} /></label>{/if}
  <BuildReusePicker selection={reuse} {recipe} history={jobs.jobs} disabled={locked} />
  <button type="button" disabled={!recipe.title.trim() || recipe.scenes.some(s => s.lines.some(l => !l.subtitle_text.trim()))} onclick={start}>{t('保存先を選んで制作開始', 'Choose destination and start')}</button>
</fieldset>
{#if error}<p role="alert">{error}</p>{/if}
{#if busy}<p role="status">{locale.text('productionChecking')}</p>{/if}
<style>fieldset { border:1px solid var(--panel-border); padding:12px; } label { display:grid; gap:6px; margin:10px 0; } input,textarea { width:100%; }</style>
