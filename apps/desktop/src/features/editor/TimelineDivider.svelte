<script lang="ts">
  import { locale } from '../../shared/i18n/locale.svelte';
  let { share = $bindable(40) }: { share?: number } = $props();
  let handle: HTMLDivElement;
  let drag: { pointer: number; y: number; share: number; height: number } | null = null;
  const clamp = (value: number) => Math.max(20, Math.min(80, value));
  function down(event: PointerEvent) {
    if (event.button !== 0) return;
    drag = { pointer: event.pointerId, y: event.clientY, share, height: handle.parentElement!.clientHeight };
    handle.setPointerCapture(event.pointerId); handle.focus(); event.preventDefault();
  }
  function move(event: PointerEvent) {
    if (drag?.pointer === event.pointerId && drag.height > 0)
      share = clamp(drag.share + (drag.y - event.clientY) * 100 / drag.height);
  }
  function end(event: PointerEvent) { if (drag?.pointer === event.pointerId) drag = null; }
  function key(event: KeyboardEvent) {
    const values: Record<string, number> = { ArrowUp: share + 5, ArrowDown: share - 5, Home: 20, End: 80 };
    if (event.key in values) { event.preventDefault(); share = clamp(values[event.key]); }
  }
</script>
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (Focusable adjustable separator implements pointer and arrow-key resizing.) -->
<div bind:this={handle} class="divider" role="separator" tabindex="0" aria-orientation="horizontal"
  aria-label={locale.text('timelineResize')} aria-valuemin="20" aria-valuemax="80" aria-valuenow={Math.round(share)}
  title={locale.text('timelineResizeHint')} onpointerdown={down} onpointermove={move}
  onpointerup={end} onpointercancel={end} onlostpointercapture={end} onkeydown={key}
  ondblclick={() => { share = 40; }}><span></span></div>
<style>
  .divider { flex:0 0 14px; margin:0 12px; display:flex; align-items:center; justify-content:center; cursor:row-resize; touch-action:none; }
  span { width:64px; height:4px; border:1px solid var(--panel-edge); border-radius:3px; background:var(--panel-header); }
  .divider:hover span,.divider:focus-visible span { background:var(--accent); }
  .divider:focus-visible { outline:2px solid var(--focus); outline-offset:-2px; }
</style>
