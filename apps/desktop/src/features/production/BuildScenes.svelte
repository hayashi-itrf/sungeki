<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { Project } from '@sungeki/api-client';
  import BuildSceneFade from './BuildSceneFade.svelte';
  import BuildDiagrams from './BuildDiagrams.svelte';
  import { newScene, type SceneInput } from './scenes';
  let { project, narration = false, portrait = false, scenes = $bindable() }: { project: Project; narration?: boolean; portrait?: boolean; scenes: SceneInput[] } = $props();
  function move(index: number, delta: number) {
    const target = index + delta;
    if (target < 0 || target >= scenes.length) return;
    const next = [...scenes]; [next[index], next[target]] = [next[target]!, next[index]!]; scenes = next;
  }
</script>
<section aria-label={locale.text('productionScenes')}>
  <h4>{locale.text('productionScenes')}</h4>
  <p>{locale.text('productionScenesHint')}</p>
  {#each scenes as scene, index (scene)}
    <fieldset>
      <legend>{locale.text('productionScene')} {index + 1}</legend>
      <label>{locale.text('productionSceneName')}<input required bind:value={scene.name} /></label>
      <label>{locale.text('productionBackground')}<select required bind:value={scene.background}><option value="">{locale.text('productionChoose')}</option>{#each project.assets.filter(a => a.probe.type === 'image') as a}<option value={a.id}>{a.name}</option>{/each}</select></label>
      <label>{locale.text('productionScript')}{narration ? locale.text('productionScriptOne') : locale.text('productionScriptTwo')}）<textarea rows="6" required bind:value={scene.script}></textarea></label>
      <p>{locale.text('productionReadingHint')}</p>
      <p>{locale.text('productionPoseHint')}</p>
      <label>{locale.text('productionGap')}<input type="number" required min="0" max="60" step="any" bind:value={scene.gapSeconds} /></label>
      <p>{locale.text('productionGapHint')}</p>
      <BuildSceneFade bind:fade={scene.fade} />
      <BuildDiagrams {portrait} {project} script={scene.script} bind:diagrams={scene.diagrams} />
      <div class="actions">
        <button type="button" disabled={index === 0} onclick={() => move(index, -1)}>{locale.text('productionScene')} {index + 1}{locale.text('productionEarlier')}</button>
        <button type="button" disabled={index === scenes.length - 1} onclick={() => move(index, 1)}>{locale.text('productionScene')} {index + 1}{locale.text('productionLater')}</button>
        <button type="button" disabled={scenes.length === 1} onclick={() => { scenes = scenes.filter((_, i) => i !== index); }}>{locale.text('productionScene')} {index + 1}{locale.text('productionRemove')}</button>
      </div>
    </fieldset>
  {/each}
  <button type="button" disabled={scenes.length >= 200} onclick={() => { const scene = newScene(`場面 ${scenes.length + 1}`); if (narration) scene.script = '左：ここにセリフを入力してください。'; scenes = [...scenes, scene]; }}>{locale.text('productionAddScene')}</button>
</section>
<style>
  fieldset { border:2px solid var(--panel-border); border-radius:6px; margin:12px 0; padding:12px; }
  label { display:grid; gap:6px; margin:10px 0; } input,select,textarea { width:100%; } .actions { display:flex; gap:8px; flex-wrap:wrap; }
</style>
