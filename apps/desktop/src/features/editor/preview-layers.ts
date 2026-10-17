import type { Clip } from '@sungeki/api-client';
export type PreviewNode = { kind: 'background' } | { kind: 'clip'; clip: Clip } |
  { kind: 'blur'; clip: Clip & { content: { type: 'blur'; sigma: number } }; children: PreviewNode[] };

// Each effect wraps only the already accumulated backdrop. No duplication of
// images or asynchronous preview requests; later clips stay outside the filter.
export function previewLayers(clips: Clip[]): PreviewNode[] {
  let nodes: PreviewNode[] = [{ kind: 'background' }];
  for (const clip of clips) {
    if (clip.content.type === 'blur') {
      if (clip.content.sigma > 0 && clip.transform.opacity > 0) {
        nodes = [{ kind: 'blur', clip: { ...clip, content: clip.content }, children: nodes }];
      }
    } else nodes.push({ kind: 'clip', clip });
  }
  return nodes;
}
