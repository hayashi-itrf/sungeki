import { describe, expect, it } from 'vitest';
import type { Project } from '@sungeki/api-client';
import { sceneCoverOpacity } from './scene-fade';
describe('scene composition fade', () => {
  it('uses scene time with half-open ends and leaves the hold clear', () => {
    const project = { scenes: [{ id: 's', name: 's', start_us: 100, duration_us: 300,
      fade: { in_us: 100, out_us: 100 } }] } as Project;
    for (const [time, opacity] of [[99,0],[100,1],[150,0.5],[200,0],[300,0],[350,0.5],[400,0]]) {
      expect(sceneCoverOpacity(project, time)).toBe(opacity);
    }
    project.scenes[0].fade = undefined;
    expect(sceneCoverOpacity(project, 100)).toBe(0);
  });
});
