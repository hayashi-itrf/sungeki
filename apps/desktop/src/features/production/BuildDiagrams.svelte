<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { Project } from '@sungeki/api-client';
  import type { DiagramInput } from './diagrams';
  import BuildDiagramRow from './BuildDiagramRow.svelte';
  import { scriptRows } from './dialogue';
  let { project, script, portrait = false, diagrams = $bindable() }: { project: Project; script: string; portrait?: boolean; diagrams: DiagramInput[] } = $props();
  const preview = $derived.by(() => {
    try { return { lines: scriptRows(script), error: '' }; }
    catch (error) { return { lines: [], error: error instanceof Error ? error.message : String(error) }; }
  });
  const lines = $derived(preview.lines);
  function move(index: number, delta: number) {
    const target = index + delta;
    if (target < 0 || target >= diagrams.length) return;
    const next = [...diagrams]; [next[index], next[target]] = [next[target]!, next[index]!]; diagrams = next;
  }
</script>
<section aria-label={locale.text('effectsSettings')}>
  <h4>{locale.text('effectsOptional')}</h4>
  <p>{locale.text('effectsHint')}</p>
  {#if preview.error}<p role="alert">{preview.error}</p>{/if}
  <details><summary>{locale.text('effectsLines')} ({lines.length})</summary><ol>{#each lines as line}<li>{line}</li>{/each}</ol></details>
  {#each diagrams as diagram, index (diagram)}
    <fieldset>
      <legend>{locale.text('effectsItem')} {index + 1}</legend>
      <BuildDiagramRow {portrait} {project} bind:row={diagrams[index]} {index} count={lines.length} />
      <button type="button" disabled={index === 0} onclick={() => move(index, -1)}>{locale.text('effectsAsset')} {index + 1}{locale.text('effectsBack')}</button>
      <button type="button" disabled={index === diagrams.length - 1} onclick={() => move(index, 1)}>{locale.text('effectsAsset')} {index + 1}{locale.text('effectsFront')}</button>
      <button type="button" onclick={() => { diagrams = diagrams.filter((_, i) => i !== index); }}>{locale.text('effectsItem')} {index + 1}{locale.text('effectsRemove')}</button>
    </fieldset>
  {/each}
  <button type="button" disabled={diagrams.length >= 200} onclick={() => { diagrams = [...diagrams, { asset: '', first: 1, last: lines.length || 1 }]; }}>{locale.text('effectsAdd')}</button>
</section>
<style>
  section { border:1px solid var(--panel-border); padding:12px; margin:12px 0; border-radius:6px; }
  fieldset { margin:12px 0; border:1px solid var(--panel-border); }
  ol { max-height:220px; overflow:auto; } li { white-space:pre-wrap; overflow-wrap:anywhere; }
</style>
