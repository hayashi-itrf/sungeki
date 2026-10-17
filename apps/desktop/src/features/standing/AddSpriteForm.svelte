<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { EditorController } from '../editor/controller.svelte';
  import SpriteFields from './SpriteFields.svelte';
  import { spriteCommands, type SpriteContent } from './create-sprite';
  let { editor, submitting }: { editor: EditorController; submitting: (id: string) => void } = $props();
  const project = (() => editor.snapshot!.project)();
  const images = project.assets.filter(a => a.probe.type === 'image' && a.probe.info.format === 'png' && a.probe.info.orientation === 1);
  let value = $state<SpriteContent>({ type: 'sprite', pose: null, character_id: null,
    sprites: { default_pose: '通常', poses: [{ name: '通常', closed_asset_id: images[0]?.id ?? '', open_asset_id: null }] } });
  let start = $state((() => editor.time)());
  let duration = $state(5), sceneId = $state('');
  async function add(event: SubmitEvent) {
    event.preventDefault();
    if (editor.busy || editor.pending || !images.length) return;
    try {
      const id = crypto.randomUUID();
      const commands = spriteCommands(project, value, start, duration, sceneId, { clip: id, track: crypto.randomUUID() });
      submitting(id); await editor.apply(commands);
    } catch (error) { editor.error = error instanceof Error ? error.message : String(error); }
  }
</script>
<form onsubmit={add} oninput={() => { editor.drafting = true; }}>
  {#if images.length === 0}
    <p role="status">{locale.text('standingSpriteImagesHint')}</p>
  {/if}
  <fieldset disabled={editor.busy || !!editor.pending || !images.length}>
    <SpriteFields changed={() => { editor.drafting = true; }} adding={true} bind:value assets={project.assets} characters={project.characters} />
    <label>{locale.text('standingStart')}<input type="number" min="0" max="604800" step="0.000001" required bind:value={start} /></label>
    <label>{locale.text('standingDuration')}<input type="number" min="0.000001" max="604800" step="0.000001" required bind:value={duration} /></label>
    <label>{locale.text('standingScene')}<select bind:value={sceneId}><option value="">{locale.text('standingNoScene')}</option>{#each project.scenes as scene}<option value={scene.id}>{scene.name}</option>{/each}</select></label>
    <p>{locale.text('standingSceneRangeHint')}</p>
    <button type="submit">{locale.text('standingSpriteAdd')}</button>
  </fieldset>
</form>
<style>
  fieldset { border:0; padding:0; } label { display:flex; flex-direction:column; gap:5px; margin:12px 0; }
  input, select { width:100%; } p { font-size:12px; }
</style>
