import { expect, test } from 'vitest';
import type { Project } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/sprite-project.json';
import cases from '../../../../../tests/fixtures/sprite-boundaries.json';
import { spritePreview } from './sprite-preview';
const fresh = () => structuredClone(fixture) as Project;

test('sample boundaries match Rust shared fixtures even when seeking backwards', () => {
  for (const entry of cases) {
    const p = fresh();
    const c = p.clips[0].content, speech = p.clips[1].content;
    if (c.type !== 'sprite' || speech.type !== 'speech' || !speech.audio?.lip_sync) throw new Error('fixture');
    speech.audio.lip_sync.sample_rate = entry.sample_rate;
    speech.audio.lip_sync.open = [{ start_frame: entry.start_frame, end_frame: entry.end_frame }];
    const before = JSON.stringify(p);
    const plan = spritePreview(p, p.clips[0]);
    expect(plan.error).toBe('');
    for (const [us, open] of [...entry.queries].reverse()) {
      expect(plan.at(Number(us))).toBe(open ? c.sprites.poses[0].open_asset_id : c.sprites.poses[0].closed_asset_id);
    }
    expect(plan.at(1_000_000)).toBeNull();
    expect(plan.at(-1)).toBeNull();
    expect(JSON.stringify(p)).toBe(before);
  }
});

test('missing analysis anywhere in standing span is explicit; muted and static poses need none', () => {
  const p = fresh();
  const c = p.clips[0].content, speech = p.clips[1].content;
  if (c.type !== 'sprite' || speech.type !== 'speech' || !speech.audio) throw new Error('fixture');
  speech.audio.lip_sync = null;
  expect(spritePreview(p, p.clips[0]).error).toContain('口パク解析');
  p.tracks[1].muted = true;
  expect(spritePreview(p, p.clips[0]).at(200000)).toBe(c.sprites.poses[0].closed_asset_id);
  p.tracks[1].muted = false; c.pose = '笑顔';
  expect(spritePreview(p, p.clips[0]).error).toBe('');
  p.tracks[0].visible = false;
  expect(spritePreview(p, p.clips[0]).at(200000)).toBeNull();
});

test('overlapping speech uses union and never ignores a missing second analysis', () => {
  const p = fresh();
  const second = structuredClone(p.clips[1]); second.id = 'second'; second.start_us = 100000;
  p.clips.push(second);
  const c = p.clips[0].content;
  if (c.type !== 'sprite' || second.content.type !== 'speech' || !second.content.audio) throw new Error('fixture');
  expect(spritePreview(p, p.clips[0]).at(350000)).toBe(c.sprites.poses[0].open_asset_id);
  second.content.audio.lip_sync = null;
  expect(spritePreview(p, p.clips[0]).error).toContain('口パク解析');
});
