<script lang="ts">
  import BuildTemplates from './BuildTemplates.svelte';
  import type { TemplateLibrary } from './template-library.svelte';
  import BuildReusePicker from './BuildReusePicker.svelte';
  import { BuildReuseSelection } from './reuse.svelte';
  import { locale } from '../../shared/i18n/locale.svelte';
  import BuildProfilePicker from './BuildProfilePicker.svelte';
  import { buildProfiles, buildConnection, savedVoicevoxConnection } from './voice-settings';
  import BuildForeword from './BuildForeword.svelte';
  import type { BuildDraft } from './drafts.svelte';
  import BuildScenes from './BuildScenes.svelte';
  import BuildMarkdown from './BuildMarkdown.svelte';
  import { onDestroy } from 'svelte';
  import { save } from '@tauri-apps/plugin-dialog';
  import type { EditorController } from '../editor/controller.svelte';
  import type { BuildJobsController } from './jobs.svelte';
  import { recipeFromForm } from './recipe';
  import { standingPreset } from './standing-preset';
  import { standingOptions } from './standing-options';
  import { hostApi } from '../../shared/host-api';
  import { rememberVoicevoxUrl } from '../speech/connection';
  let { editor, jobs, templates, draft = $bindable() }: { editor: EditorController; jobs: BuildJobsController; templates: TemplateLibrary; draft: BuildDraft } = $props();
  const reuse = new BuildReuseSelection(hostApi);
  let templateBusy = $state(false);
  let busy = $state(false), error = $state('');
  let alive = true;
  onDestroy(() => { alive = false; reuse.dispose(); });
  const portrait = $derived(draft.preset === 'narration-portrait-v1');
  const narration = $derived(portrait || draft.preset === 'narration-wide-v1');
  const project = $derived(editor.snapshot?.project);
  const usesVoicevox = $derived(draft.people.slice(0, narration ? 1 : 2).some(p => project?.characters.find(c => c.id === p.character)?.voice.type === 'voicevox'));
  const disabled = $derived(templateBusy || busy || reuse.busy || jobs.busy || !!jobs.recoveryError || !!jobs.pending || editor.busy || editor.drafting || !!editor.pending);
  const reuseRecipe = $derived.by(() => { try { return project ? recipeFromForm(project, draft) : null; } catch { return null; } });
  const savedConnection = $derived(reuseRecipe && savedVoicevoxConnection(reuseRecipe, reuse.source
    ? { parent_job_id: reuse.source.job_id, speech: reuse.choices } : undefined, reuse.source));
  function label(id: string) {
    const content = project?.clips.find(c => c.id === id)?.content;
    if (content?.type === 'vrm') {
      const model = content.standing.model_asset_id;
      return locale.text('productionVrmOption').replace('{name}', () => project?.assets.find(a => a.id === model)?.name ?? locale.text('productionModel'));
    }
    if (content?.type === 'sprite') return locale.text('productionSpriteOption').replace('{name}', () => content.pose ?? content.sprites.default_pose);
    const asset = content && 'asset_id' in content ? content.asset_id : null;
    return project?.assets.find(a => a.id === asset)?.name ?? id;
  }
  async function start(event: SubmitEvent) {
    event.preventDefault(); if (disabled || !project) return;
    busy = true; error = '';
    const source = editor.snapshot;
    const current = () => alive && editor.snapshot?.session_id === source?.session_id && editor.snapshot?.revision === source?.revision;
    try {
      const recipe = recipeFromForm(project, draft);
      const reuse_from = reuse.request(recipe);
      const reuseSource = reuse.source;
      const managed_profiles = buildProfiles(recipe, draft.people);
      const url = draft.baseUrl;
      const title = draft.title;
      const path = await save({ title: locale.text('productionSaveTitle'), defaultPath: `${title.replace(/[\\/:*?"<>|]/g, '_')}.vvm.json`, filters: [{ name: locale.text('productionProject'), extensions: ['json'] }] });
      if (!path || !alive) return;
      if (!current()) throw new Error(locale.text('productionChanged'));
      const connection = await buildConnection(hostApi, recipe, url, reuse_from, reuseSource);
      if (!current()) throw new Error(locale.text('productionChanged'));
      if (connection.base_url) rememberVoicevoxUrl(connection.base_url);
      await jobs.start({ recipe, managed_profiles, project_path: path, ...connection, reuse_from });
    } catch (e) { error = e && typeof e === 'object' && 'message' in e ? String(e.message) : String(e); }
    finally { busy = false; }
  }
</script>
<details bind:open={draft.expanded}><summary>{locale.text('productionFormTitle')}</summary>
  <p>{locale.text('productionIntro')} {portrait ? locale.text('productionPortrait') : locale.text('productionLandscape')}{locale.text('productionTiming')}</p>
  <p>{locale.text('productionEngineHint')}</p>
  {#if project}<form onsubmit={start}><fieldset {disabled}>
    <label>{locale.text('productionName')}<input required bind:value={draft.title} /></label>
    <label>{locale.text('productionFormat')}<select bind:value={draft.preset}><option value="dialogue-wide-v1">{locale.text('productionDialogue')}</option><option value="narration-wide-v1">{locale.text('productionNarrationWide')}</option><option value="narration-portrait-v1">{locale.text('productionNarrationPortrait')}</option></select></label>
    {#if narration}<p>{locale.text('productionNarratorHint')}{portrait ? locale.text('productionCenterBottom') : locale.text('productionLeft')}{locale.text('productionNarrationHint')}</p>{/if}
    {#if portrait}<p>{locale.text('productionPortraitHint')}</p>{/if}
    {#each draft.people.slice(0, narration ? 1 : 2) as person, index}
      <h4>{narration ? locale.text('productionNarrator') : index === 0 ? locale.text('productionLeftSpeaker') : locale.text('productionRightSpeaker')}{locale.text('productionStandingHeading')}</h4>
      <label>{locale.text('productionSpeaker')}<select required bind:value={person.character} onchange={() => { person.standing = ''; person.profile = undefined; }}><option value="">{locale.text('productionChoose')}</option>{#each project.characters as c}<option value={c.id}>{c.name}</option>{/each}</select></label>
      {@const selectedCharacter = project.characters.find(c => c.id === person.character)}
      {#if selectedCharacter && selectedCharacter.voice.type !== 'voicevox' && selectedCharacter.voice.type !== 'local_http'}
        {#key `${selectedCharacter.id}:${selectedCharacter.voice.type}`}
          <BuildProfilePicker engine={selectedCharacter.voice.type} {person} {disabled} />
        {/key}
      {/if}
      <label>{locale.text('productionStanding')}<select required disabled={!person.character} bind:value={person.standing}><option value="">{person.character ? locale.text('productionChoose') : locale.text('productionChooseSpeaker')}</option>{#each standingOptions(project, person.character) as c}<option value={c.id}>{label(c.id)}（{c.id.slice(0, 6)}）</option>{/each}</select></label>
      {#if project.clips.find(c => c.id === person.standing)?.content.type === 'psd'}
        {@const preset = standingPreset(project.clips.find(c => c.id === person.standing)?.content, selectedCharacter?.voice.type)}
        {#if preset}<p>{locale.text('productionPreset')}{preset.source_name}／{preset.pose_name}{locale.text('productionPresetHint')}</p>
        {:else}<p role="alert">{locale.text('productionUnsupportedPsd')}</p>{/if}
      {:else if project.clips.find(c => c.id === person.standing)?.content.type === 'vrm'}
        <p>{locale.text('productionVrmHint')}</p>
      {:else if project.clips.find(c => c.id === person.standing)?.content.type === 'sprite'}
        <p>{locale.text('productionSpriteHint')}</p>
      {/if}
    {/each}
    <label>BGM<select bind:value={draft.bgm}><option value="">{locale.text('productionNone')}</option>{#each project.clips.filter(c => c.content.type === 'audio') as c}<option value={c.id}>{label(c.id)}（{c.id.slice(0, 6)}）</option>{/each}</select></label>
    {#if draft.bgm}<label><input type="checkbox" bind:checked={draft.fitBgmVolumePoints} />{locale.text('productionFitBgm')}</label><p>{locale.text('productionFitBgmHint')}</p>{/if}
    <BuildForeword bind:foreword={draft.foreword} />
    <BuildMarkdown {narration} {disabled} bind:scenes={draft.scenes} bind:source={() => draft.markdownSource ?? '', value => draft.markdownSource = value} />
    <BuildScenes {project} {narration} {portrait} bind:scenes={draft.scenes} />
    {#if usesVoicevox && !savedConnection}<label>{locale.text('productionVoicevox')}<input type="url" required bind:value={draft.baseUrl} /></label>{/if}
    <BuildReusePicker selection={reuse} recipe={reuseRecipe} history={jobs.jobs} {disabled} />
    <button>{locale.text('productionStart')}</button>
  </fieldset></form>{:else}<p>{locale.text('productionOpen')}</p>{/if}
  <BuildTemplates library={templates} currentRecipe={reuseRecipe} {jobs} {disabled} onbusy={v => templateBusy = v} />
  {#if busy}<p role="status">{locale.text('productionChecking')}</p>{/if}
  {#if error}<p role="alert">{error}</p>{/if}
</details>
<style>
  details { margin:12px 0; padding:14px; border:2px solid var(--panel-border); border-radius:6px; }
  label { display:grid; gap:6px; margin:10px 0; } fieldset { border:0; padding:0; } input,select { width:100%; } input[type="checkbox"] { width:auto; justify-self:start; }
</style>
