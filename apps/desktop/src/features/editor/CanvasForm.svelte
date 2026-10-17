<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { Command } from '@sungeki/api-client';
  import ColorInput from '../../shared/ColorInput.svelte';
  import type { EditorController } from './controller.svelte';
  let { editor }: { editor: EditorController } = $props();
  const original = (() => editor.snapshot!.project.canvas)();
  let resizeLayout = $state(false);
  let width = $state(original.width);
  let height = $state(original.height);
  let numerator = $state(original.fps.numerator);
  let denominator = $state(original.fps.denominator);
  let background = $state(original.background);
  function size(w: number, h: number) { width = w; height = h; editor.drafting = true; }
  function rate(n: number, d = 1) { numerator = n; denominator = d; editor.drafting = true; }
  async function apply(event: SubmitEvent) {
    event.preventDefault();
    if ([width, height, numerator, denominator].some(v => !Number.isInteger(v) || v < 1 || v > 4294967295)) return;
    const commands: Command[] = [];
    if (resizeLayout) commands.push({ type: 'resize_layout', width, height });
    commands.push({ type: 'set_canvas', canvas: { width, height, fps: { numerator, denominator }, background } });
    await editor.apply(commands);
  }
</script>
<form onsubmit={apply} oninput={() => { editor.drafting = true; }}>
  <p>{locale.text('canvasHint')}</p>
  <fieldset disabled={editor.busy || !!editor.pending}>
    <h3>{locale.text('canvasSize')}</h3>
    <div class="presets">
      <button type="button" onclick={() => size(1920, 1080)}>{locale.text('canvasLandscape')}</button>
      <button type="button" onclick={() => size(1080, 1920)}>{locale.text('canvasPortrait')}</button>
      <button type="button" onclick={() => size(1080, 1080)}>{locale.text('canvasSquare')}</button>
    </div>
    <div class="numbers">
      <label>{locale.text('canvasWidth')}<input type="number" min="1" max="4294967295" step="1" required bind:value={width} /></label>
      <label>{locale.text('canvasHeight')}<input type="number" min="1" max="4294967295" step="1" required bind:value={height} /></label>
    </div>
    <label class="fit"><input type="checkbox" bind:checked={resizeLayout} />{locale.text('canvasFitLayout')}</label>
    <p>{locale.text('canvasFitHint')}</p>
    <h3>{locale.text('canvasRate')}</h3>
    <div class="presets">
      <button type="button" onclick={() => rate(24)}>24 fps</button>
      <button type="button" onclick={() => rate(30)}>30 fps</button>
      <button type="button" onclick={() => rate(60)}>60 fps</button>
      <button type="button" onclick={() => rate(30000, 1001)}>29.97 fps</button>
      <button type="button" onclick={() => rate(60000, 1001)}>59.94 fps</button>
    </div>
    <p>{locale.text('canvasCurrent')}{Number.isFinite(numerator / denominator) ? (numerator / denominator).toFixed(3) : locale.text('canvasUnset')} fps</p>
    <details><summary>{locale.text('canvasExactRate')}</summary>
      <div class="numbers">
        <label>{locale.text('canvasNumerator')}<input type="number" min="1" max="4294967295" step="1" required bind:value={numerator} /></label>
        <label>{locale.text('canvasDenominator')}<input type="number" min="1" max="4294967295" step="1" required bind:value={denominator} /></label>
      </div>
      <p>{locale.text('canvasRateHint')}</p>
    </details>
    <ColorInput label={locale.text('canvasBackground')} bind:value={background} />
    <button type="submit">{locale.text('inspectorApply')}</button>
    {#if editor.drafting}<button type="button" onclick={() => editor.acceptIncoming()}>{locale.text('inspectorDiscard')}</button>{/if}
  </fieldset>
</form>
<style>
  fieldset { border:0; padding:0; }
  .presets { display:flex; flex-wrap:wrap; gap:6px; margin-bottom:12px; }
  .numbers { display:grid; grid-template-columns:1fr 1fr; gap:12px; }
  label { display:flex; flex-direction:column; gap:4px; margin-bottom:12px; }
  input { width:100%; }
  .fit { flex-direction:row; align-items:center; gap:8px; }
  .fit input { width:auto; }
  h3 { margin-top:16px; }
  p { font-size:12px; }
  fieldset > button { margin-right:8px; }
</style>
