<script lang="ts">
  import { tick, untrack, onDestroy } from 'svelte';
  import { locale } from '../../shared/i18n/locale.svelte';
  import ColorInput from '../../shared/ColorInput.svelte';
  import { visualCommands, visualDraft } from './quick-visual';
  import type { EditorController } from './controller.svelte';
  let { editor, editing = $bindable(false) }: { editor: EditorController; editing?: boolean } = $props();
  const clip = $derived(editor.snapshot?.project.clips.find(c => c.id === editor.selected));
  let draft = $state((() => visualDraft(editor.snapshot!.project.clips.find(c => c.id === editor.selected)!))());
  let form: HTMLFormElement;
  let tabTarget: HTMLElement | null | undefined;
  function keyboard(event: KeyboardEvent) {
    if (!form?.contains(document.activeElement)) return;
    if (event.key === 'Tab') {
      const fields = [...form.querySelectorAll<HTMLElement>('input, select, button')]
        .filter(el => !el.matches(':disabled'));
      const index = fields.indexOf(document.activeElement as HTMLElement);
      tabTarget = fields[index + (event.shiftKey ? -1 : 1)] ?? null;
    } else if (event.key === 'Escape' && editing && !editor.busy && !editor.pending) {
      event.preventDefault(); editor.acceptIncoming();
    }
  }
  $effect(() => {
    const current = clip, generation = editor.draftGeneration;
    untrack(() => { if (current) { draft = visualDraft(current); editing = false; } });
  });
  onDestroy(() => { editing = false; });
  function begin() { tabTarget = undefined; editing = true; editor.drafting = true; }
  async function commit(event?: Event) {
    event?.preventDefault();
    await tick(); // Svelte bindings run after the input/change listener.
    if (!clip || !editing || editor.busy || editor.pending || editor.incoming) return;
    if (!form.reportValidity()) return;
    const commands = visualCommands(clip, draft);
    if (!commands) return;
    if (!commands.length) { editing = false; editor.acceptIncoming(); return; }
    const focused = tabTarget === undefined ? document.activeElement : tabTarget;
    tabTarget = undefined;
    await editor.apply(commands);
    await tick();
    if (focused instanceof HTMLElement && focused.isConnected && (document.activeElement === document.body || document.activeElement === document.documentElement)) focused.focus();
  }
</script>
<svelte:window onkeydown={keyboard} />
<form bind:this={form} onsubmit={commit} oninput={begin} onchange={commit}>
  <h3>{locale.current === 'en' ? 'Appearance' : '見た目の調整'}</h3>
  <p>{locale.current === 'en' ? 'Numbers and colors update when confirmed or when you leave the field. Choices update immediately.' : '数値・色は入力を確定するか欄を離れると反映します。選択項目は選ぶと反映します。'}</p>
  {#if editor.drafting && !editing}<p>{locale.current === 'en' ? 'Apply or discard the other pending edits first.' : '他の欄の入力を適用するか取り消すと、調整できます。'}</p>{/if}
  <fieldset disabled={editor.busy || !!editor.pending || !!editor.incoming || (editor.drafting && !editing)}>
    <div class="numbers">
      <label>X<input type="number" step="any" required bind:value={draft.x} /></label>
      <label>Y<input type="number" step="any" required bind:value={draft.y} /></label>
      <label>{locale.text('inspectorWidth')}<input type="number" min="0.001" step="any" required bind:value={draft.width} /></label>
      <label>{locale.text('inspectorHeight')}<input type="number" min="0.001" step="any" required bind:value={draft.height} /></label>
      <label>{locale.text('inspectorOpacity')}<input type="number" min="0" max="1" step="0.01" required bind:value={draft.opacity} /></label>
    </div>
    {#if clip && ['image', 'psd', 'video', 'sprite', 'vrm'].includes(clip.content.type)}
      <label class="check"><input type="checkbox" bind:checked={draft.flip} />{locale.text('inspectorFlip')}</label>
    {/if}
    {#if clip?.content.type === 'text'}
      <label>{locale.text('inspectorFontSize')}<input type="number" min="0.001" step="any" required bind:value={draft.fontSize} /></label>
      <ColorInput label={locale.text('inspectorTextColor')} bind:value={draft.color} />
    {:else if clip?.content.type === 'shape'}
      <label>{locale.text('inspectorShapeKind')}<select bind:value={draft.shape}><option value="rectangle">{locale.text('inspectorRectangle')}</option><option value="ellipse">{locale.text('inspectorEllipse')}</option></select></label>
      <ColorInput label={locale.text('inspectorFill')} bind:value={draft.color} />
    {:else if clip?.content.type === 'blur'}
      <label>{locale.text('inspectorBlurStrength')}<input type="number" min="0" max="128" step="0.1" required bind:value={draft.sigma} /></label>
    {/if}
    {#if editing && editor.error}<button type="submit">{locale.current === 'en' ? 'Retry change' : '変更を再試行'}</button>{/if}
  </fieldset>
  {#if editing}<button type="button" disabled={editor.busy || !!editor.pending} onpointerdown={event => event.preventDefault()} onclick={() => editor.acceptIncoming()}>{locale.text('inspectorDiscard')}</button>{/if}
</form>
<style>
  form { border-bottom:1px solid var(--panel-border); padding-bottom:12px; margin-bottom:20px; }
  h3 { margin:0 0 8px; } p { font-size:12px; line-height:1.5; }
  fieldset { border:0; padding:0; min-width:0; }
  .numbers { display:grid; grid-template-columns:1fr 1fr; gap:8px; }
  label { display:flex; flex-direction:column; gap:4px; margin-bottom:10px; font-size:13px; }
  input, select { width:100%; box-sizing:border-box; }
  .check { flex-direction:row; } .check input { width:auto; }
</style>
