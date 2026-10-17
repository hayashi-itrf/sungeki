import { expect, test } from 'vitest';
import type { Project } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/video-clip-project.json';
import { newReaction, reactionFromForm, type ReactionInput } from './reaction';
import { diagramsFromForm, selectDiagram } from './diagrams';
import { BuildDrafts } from './drafts.svelte';
function project(): Project {
  const p = structuredClone(fixture) as Project;
  p.assets.push({ ...p.assets[0]!, id: 'image', probe: { type: 'image' } } as unknown as Project['assets'][number]);
  return p;
}
test('reaction form translates units and shares speech range with a diagram without editing source', () => {
  const p = project(), before = structuredClone(p), reaction = newReaction(false);
  const result = diagramsFromForm(p, [
    { asset: 'image', first: 1, last: 3 },
    { asset: 'image', first: 2, last: 2, reaction },
  ], 3);
  expect(result[1]).toMatchObject({ first_line: 1, last_line: 1, reaction: {
    center_x: 960, center_y: 300, start_scale: 0.5, fade: { in_us: 150000, out_us: 150000 },
  } });
  result[1]!.reaction!.center_x = 12;
  expect(reaction.x).toBe(960); expect(p).toEqual(before);
  expect(newReaction(true).x).toBe(540);
});
test('empty invalid and excessive values are rejected rather than converted to zeros', () => {
  for (const patch of [{ x: undefined }, { y: NaN }, { width: 0 }, { height: -1 }, { scale: 0 }, { scale: 101 }, { fadeIn: -1 }, { fadeOut: Infinity }, { fadeIn: 604801 }]) {
    expect(() => reactionFromForm({ ...newReaction(false), ...patch } as ReactionInput)).toThrow('リアクション');
  }
});
test('image changes preserve effect, video changes clear it, and stale video effects are rejected', () => {
  const p = project(), row = { asset: 'image', first: 1, last: 1, reaction: newReaction(false) };
  const next = selectDiagram(p, row, 'image');
  expect(next.reaction).toEqual(row.reaction); expect(next.reaction).not.toBe(row.reaction);
  const video = selectDiagram(p, row, p.assets[0]!.id);
  expect(video.reaction).toBeUndefined();
  expect(() => diagramsFromForm(p, [{ ...video, reaction: row.reaction }], 1)).toThrow('画像');
});
test('session drafts retain reaction inputs without sharing them with another project', () => {
  const drafts = new BuildDrafts(() => '');
  drafts.forSession('a').scenes[0]!.diagrams.push({ asset: 'image', first: 1, last: 1, reaction: newReaction(true) });
  expect(drafts.forSession('b').scenes[0]!.diagrams).toEqual([]);
  expect(drafts.forSession('a').scenes[0]!.diagrams[0]!.reaction?.x).toBe(540);
});
