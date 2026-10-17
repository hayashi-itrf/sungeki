<script lang="ts">
  import type { BuildRecipe, BuildVideoJob } from '@sungeki/api-client';
  import { locale } from '../../shared/i18n/locale.svelte';
  import { BuildReuseSelection, matchingAudio, reuseTargets } from './reuse.svelte';
  let { selection, recipe, history, disabled }: {
    selection: BuildReuseSelection; recipe: BuildRecipe | null; history: BuildVideoJob[]; disabled: boolean;
  } = $props();
  let selected = $state('');
  $effect(() => { selection.sync(recipe); });
  const count = $derived(Object.keys(selection.choices).length);
</script>
<details>
  <summary>{locale.text('buildReuseTitle')}（{count}）</summary>
  <p>{locale.text('buildReuseHint')}</p>
  <label>{locale.text('buildReuseSource')}<select bind:value={selected} disabled={disabled || selection.busy} onchange={() => selection.load(selected)}>
    <option value="">{locale.text('buildReuseNone')}</option>
    {#each history.filter(job => ['succeeded', 'failed', 'cancelled', 'interrupted'].includes(job.phase)) as job}
      <option value={job.job_id}>{job.title}</option>
    {/each}
  </select></label>
  {#if !history.length}<p>{locale.text('buildReuseHistory')}</p>{/if}
  {#if selection.busy}<p role="status">{locale.text('buildReuseLoading')}</p>{/if}
  {#if selection.error}<p role="alert">{selection.error}</p><button type="button" disabled={disabled || selection.busy} onclick={() => selection.load(selected)}>{locale.text('buildRefresh')}</button>{/if}
  {#if selection.changed}<p role="status">{locale.text('buildReuseChanged')}</p>{/if}
  {#if selection.source}
    {#if recipe}
      {#each reuseTargets(recipe) as line, index}
        {@const candidates = matchingAudio(recipe, index, selection.source)}
        <label>{index + 1}. {line.characterName}：{line.text}
          <select disabled={disabled || !candidates.length} value={selection.choices[index] ?? ''} onchange={(event) => selection.choose(recipe, index, event.currentTarget.value)}>
            <option value="">{locale.text('buildReuseSynthesize')}</option>
            {#each candidates as candidate}<option value={candidate.index}>{locale.text('buildReuseRecording')} {candidate.index + 1}：{candidate.subtitle_text}</option>{/each}
          </select>
        </label>
      {/each}
      <p>{locale.text('buildReuseValidation')}</p>
    {:else}<p>{locale.text('buildReuseCompleteForm')}</p>{/if}
  {/if}
</details>
<style>
  details { margin:16px 0; padding:14px; border:2px solid var(--panel-border); border-radius:8px; background:var(--surface); }
  label { display:grid; gap:6px; margin:12px 0; overflow-wrap:anywhere; }
  select { width:100%; min-width:0; }
</style>
