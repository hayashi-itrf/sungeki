import { expect, test } from 'vitest';
import type { Project } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/scene-evaluation.json';
import { previewClips } from './preview-evaluation';

test('shares Rust scene visibility, half-open boundary and z-order rules', () => {
  const project = fixture.project as Project;
  const before = structuredClone(project);
  for (const sample of fixture.cases) {
    expect(previewClips(project, sample.time_us).map(clip => clip.id)).toEqual(sample.clip_ids);
  }
  expect(project).toEqual(before);
});

test('rejects invalid cursor times without displaying arbitrary clips', () => {
  for (const time of [-1, NaN, Infinity, 1.5, Number.MAX_SAFE_INTEGER + 1]) {
    expect(previewClips(fixture.project as Project, time)).toEqual([]);
  }
});

test('audio material has no visual layer even on a visible track', () => {
  const project = structuredClone(fixture.project) as Project;
  project.clips[0].content = { type: 'audio', asset_id: 'a', playback: {
    source_start_us: 0, source_duration_us: 100000, looped: true, gain_db: -18,
    volume_points: [], fade_in_us: 0, ducking: null, fade_out_us: 0,
  } };
  expect(previewClips(project, project.clips[0].start_us).map(c => c.id)).not.toContain(project.clips[0].id);
});

test('fades only pixels, retains source state, and starts relative to the clip', () => {
  const project = structuredClone(fixture.project) as Project;
  const clip = project.clips[0];
  clip.start_us = 100000; clip.duration_us = 3000000;
  clip.transform.opacity = .8;
  clip.transform.fade = { in_us: 1000000, out_us: 1000000 };
  const before = structuredClone(project);
  for (const [time, opacity] of [[100000, 0], [600000, .4], [1600000, .8], [2600000, .4]]) {
    expect(previewClips(project, time).find(c => c.id === clip.id)?.transform.opacity).toBeCloseTo(opacity);
  }
  expect(project).toEqual(before);
});
