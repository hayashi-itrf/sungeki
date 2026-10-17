<script lang="ts">
  import type { SubtitleStyle, Transform } from '@sungeki/api-client';
  let { text, style, transform: t }: { text: string; style: SubtitleStyle; transform: Transform } = $props();
  let stroke = $derived(style.outline?.width ?? 0);
</script>
<foreignObject x={t.x} y={t.y} width={t.width} height={t.height} overflow="visible">
  <div style:font-size={`${style.font_size}px`} style:color={style.color}
    style:text-align={style.layout?.alignment ?? 'left'} style:white-space={style.layout?.wrap ? 'pre-wrap' : 'pre'}
    style:padding={`${stroke}px`} style:--stroke={`${stroke * 2}px ${style.outline?.color ?? 'transparent'}`}>{text}</div>
</foreignObject>
<style>
  div { box-sizing:border-box; width:100%; line-height:1.3; overflow-wrap:anywhere;
    -webkit-text-stroke:var(--stroke); paint-order:stroke fill; }
</style>
