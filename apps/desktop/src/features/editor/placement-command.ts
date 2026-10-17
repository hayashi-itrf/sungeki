import type { Clip, Command, Transform } from '@sungeki/api-client';

type PlacementDraft = Pick<Transform, 'x' | 'y' | 'width' | 'height' | 'opacity'> & {
  flipX: boolean; fadeIn: number; fadeOut: number;
};

/** Keep optional fields and motion intact when only text/audio settings change. */
export function placementCommand(clip: Clip, draft: PlacementDraft): Command | null {
  if (clip.content.type === 'audio') return null;
  const before = clip.transform;
  const after = { ...before };
  let changed = false;
  for (const key of ['x', 'y', 'width', 'height', 'opacity'] as const) {
    if (draft[key] !== before[key]) { after[key] = draft[key]; changed = true; }
  }
  if (draft.flipX !== (before.flip_x ?? false)) {
    after.flip_x = draft.flipX; changed = true;
  }
  const in_us = Math.round(draft.fadeIn * 1e6), out_us = Math.round(draft.fadeOut * 1e6);
  if (in_us !== (before.fade?.in_us ?? 0) || out_us !== (before.fade?.out_us ?? 0)) {
    if (in_us || out_us) after.fade = { in_us, out_us };
    else delete after.fade;
    changed = true;
  }
  return changed ? { type: 'set_transform', clip_id: clip.id, transform: after } : null;
}
