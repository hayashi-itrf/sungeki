<script lang="ts">
  import { containDialogFocus } from '../../shared/components/dialog-focus';
  import { locale } from '../../shared/i18n/locale.svelte';
  import ColorInput from '../../shared/ColorInput.svelte';
  import type { SessionSnapshot, TextAlignment } from '@sungeki/api-client';
  import type { EditorController } from '../editor/controller.svelte';
  import { hostApi } from '../../shared/host-api';
  import { EditPlan } from '../scenes/edit-plan.svelte';
  import EditReview from '../scenes/EditReview.svelte';
  import EditorNotices from '../editor/EditorNotices.svelte';
  let { editor }: { editor: EditorController } = $props();
  let dialog: HTMLDialogElement;
  let observed = $state.raw<SessionSnapshot | null>(null);
  let characterId = $state('');
  let alignment = $state<TextAlignment>('left'), wrap = $state(false);
  let size = $state(64), color = $state('#ffffff');
  let enabled = $state(false), width = $state(3), outlineColor = $state('#000000');
  const plan = new EditPlan(hostApi);
  let stale = $derived(!!observed && (observed.revision !== editor.snapshot?.revision || observed.session_id !== editor.snapshot?.session_id));
  let disabled = $derived(stale || editor.busy || !!editor.pending || editor.drafting);
  function choose() {
    const style = observed?.project.characters.find(c => c.id === characterId)?.subtitle_style;
    if (!style) return;
    alignment = style.layout?.alignment ?? 'left'; wrap = style.layout?.wrap ?? false;
    size = style.font_size; color = style.color; enabled = !!style.outline;
    width = style.outline?.width ?? 3; outlineColor = style.outline?.color ?? '#000000';
  }
  function refresh() {
    plan.cancel(); observed = editor.snapshot;
    characterId = observed?.project.characters[0]?.id ?? ''; choose();
  }
  async function preview(event: SubmitEvent) {
    event.preventDefault();
    const character = observed?.project.characters.find(c => c.id === characterId);
    if (disabled || !observed || !character) return;
    await plan.prepare(observed, [{ type: 'set_character', character: { ...character,
      subtitle_style: { layout: { alignment, wrap }, font_size: size, color, outline: enabled ? { width, color: outlineColor } : null } } }]);
  }
  async function confirm() {
    if (disabled || plan.busy || plan.stale(editor.snapshot) || !plan.request || !plan.preview || plan.error) return;
    await editor.request(plan.request);
    if (!editor.error && !editor.pending) { plan.cancel(); dialog.close(); }
  }
</script>
<button disabled={editor.busy || !!editor.pending || editor.drafting || !editor.snapshot?.project.characters.length} onclick={() => { refresh(); dialog.showModal(); }}>{locale.text('subtitleStyleTitle')}</button>
<dialog use:containDialogFocus bind:this={dialog} onclose={() => plan.cancel()} aria-label={locale.text('subtitleStyleTitle')}>
  <header><h2>{locale.text('subtitleStyleTitle')}</h2><button onclick={() => dialog.close()}>{locale.text('close')}</button></header>
  <EditorNotices {editor} />
  <p>{locale.text('subtitleStyleDescription')}</p>
  {#if stale}<p role="alert">{locale.text('subtitleStyleStale')}</p>{/if}
  <button disabled={editor.busy || !!editor.pending} onclick={refresh}>{locale.text('subtitleStyleRefresh')}</button>
  {#if observed && plan.request}
    <EditReview {plan} project={observed.project} stale={stale || plan.stale(editor.snapshot)} {disabled} {confirm} />
  {:else}
    <form onsubmit={preview}><fieldset disabled={disabled}>
      <label>{locale.text('subtitleStyleSpeaker')}<select bind:value={characterId} onchange={choose}>{#each observed?.project.characters ?? [] as c}<option value={c.id}>{c.name}</option>{/each}</select></label>
      <label>{locale.text('subtitleStyleSize')}<input type="number" min="1" max="4096" step="1" required bind:value={size} /></label>
      <ColorInput label={locale.text('subtitleStyleColor')} bind:value={color} />
      <label>{locale.text('subtitleStyleAlignment')}<select bind:value={alignment}><option value="left">{locale.text('subtitleStyleLeft')}</option><option value="center">{locale.text('subtitleStyleCenter')}</option><option value="right">{locale.text('subtitleStyleRight')}</option></select></label>
      <label><input type="checkbox" bind:checked={wrap} /> {locale.text('subtitleStyleWrap')}</label>
      <label><input type="checkbox" bind:checked={enabled} /> {locale.text('subtitleStyleOutline')}</label>
      {#if enabled}
        <label>{locale.text('subtitleStyleWidth')}<input type="number" min="1" max="32" step="1" required bind:value={width} /></label>
        <ColorInput label={locale.text('subtitleStyleOutlineColor')} bind:value={outlineColor} />
      {/if}
      <p>{locale.text('subtitleStyleOverflow')}</p>
      <button disabled={!characterId}>{locale.text('subtitleStyleReview')}</button>
    </fieldset></form>
  {/if}
</dialog>
<style>
  header { display:flex; align-items:center; justify-content:space-between; gap:24px; }
  dialog { max-width:700px; }
  fieldset { border:0; padding:12px 0; }
  label { display:block; margin:12px 0; }
</style>
