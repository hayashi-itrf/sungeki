<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  import type { EditorController } from '../editor/controller.svelte';
  let { editor }: { editor: EditorController } = $props();
  const original = (() => editor.snapshot!.project.audio_mix.normalization)();
  let enabled = $state(original !== null);
  // No automatic target: the user explicitly enters it when enabling adjustment.
  let target = $state<number | undefined>(original?.target_lufs);
  let peak = $state<number | undefined>(original?.true_peak_db);
  async function apply(event: SubmitEvent) {
    event.preventDefault();
    if (enabled && (target === undefined || peak === undefined || !Number.isFinite(target)
      || !Number.isFinite(peak) || target <= -70 || target > 0 || peak < -20 || peak > 0)) return;
    await editor.apply([{ type: 'set_audio_mix', audio_mix: {
      normalization: enabled ? { target_lufs: target!, true_peak_db: peak! } : null,
    } }]);
  }
</script>
<form onsubmit={apply} oninput={() => { editor.drafting = true; }}>
  <p>{locale.text('mixDescription')}</p>
  <fieldset disabled={editor.busy || !!editor.pending}>
    <label class="toggle"><input type="checkbox" bind:checked={enabled} />{locale.text('mixEnable')}</label>
    {#if enabled}
      <label>{locale.text('mixTarget')}<input type="number" min="-69.9" max="0" step="0.1" required bind:value={target} /></label>
      <label>{locale.text('mixPeak')}<input type="number" min="-20" max="0" step="0.1" required bind:value={peak} /></label>
      <p>{locale.text('mixLimits')}</p>
      <p>{locale.text('mixVerify')}</p>
    {/if}
    <p>{locale.text('mixPreview')}</p>
    <button type="submit">{locale.text('mixApply')}</button>
    {#if editor.drafting}<button type="button" onclick={() => editor.acceptIncoming()}>{locale.text('mixDiscard')}</button>{/if}
  </fieldset>
</form>
<style>
  fieldset { border:0; padding:0; } label { display:flex; flex-direction:column; gap:6px; margin:12px 0; }
  .toggle { flex-direction:row; align-items:center; } p { font-size:12px; line-height:1.7; }
  button { margin-right:8px; }
</style>
