import type { Clip, Command } from '@sungeki/api-client';

export function visualDraft(clip: Clip) {
  const c = clip.content;
  return { x: clip.transform.x, y: clip.transform.y, width: clip.transform.width,
    height: clip.transform.height, opacity: clip.transform.opacity, flip: clip.transform.flip_x ?? false,
    fontSize: c.type === 'text' ? c.font_size : 64,
    color: c.type === 'text' ? c.color : c.type === 'shape' ? c.fill : '#000000',
    shape: c.type === 'shape' ? c.shape : 'rectangle' as const,
    sigma: c.type === 'blur' ? c.sigma : 18 };
}
export type VisualDraft = ReturnType<typeof visualDraft>;

// Only these independent fields are auto-committed. Timing/content drafts stay separate.
export function visualCommands(clip: Clip, draft: VisualDraft): Command[] | null {
  const { x, y, width, height, opacity, fontSize, sigma } = draft;
  if (![x, y, width, height, opacity].every(Number.isFinite)
    || width <= 0 || height <= 0 || opacity < 0 || opacity > 1) return null;
  const c = clip.content;
  if ((c.type === 'text' || c.type === 'shape') && !/^#[0-9a-f]{6}$/i.test(draft.color)) return null;
  if (c.type === 'text' && (!Number.isFinite(fontSize) || fontSize <= 0)) return null;
  if (c.type === 'blur' && (!Number.isFinite(sigma) || sigma < 0 || sigma > 128)) return null;
  const commands: Command[] = [], before = visualDraft(clip);
  if (['x', 'y', 'width', 'height', 'opacity', 'flip'].some(k => before[k as keyof VisualDraft] !== draft[k as keyof VisualDraft])) {
    const transform = { ...clip.transform, x, y, width, height, opacity };
    if (draft.flip !== before.flip) transform.flip_x = draft.flip;
    commands.push({ type: 'set_transform', clip_id: clip.id, transform });
  }
  if (c.type === 'text' && (fontSize !== c.font_size || draft.color !== c.color))
    commands.push({ type: 'set_text_style', clip_id: clip.id, font_size: fontSize, color: draft.color });
  if (c.type === 'shape' && (draft.shape !== c.shape || draft.color !== c.fill))
    commands.push({ type: 'set_shape', clip_id: clip.id, shape: draft.shape, fill: draft.color });
  if (c.type === 'blur' && sigma !== c.sigma) commands.push({ type: 'set_blur', clip_id: clip.id, sigma });
  return commands;
}
