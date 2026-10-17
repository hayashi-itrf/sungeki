import { expect, test } from 'vitest';
import { render } from 'svelte/server';
import type { Project } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/scene-evaluation.json';
import PreviewVisual from './PreviewVisual.svelte';

test('default text preserves spaces and empty lines just like explicit left/no-wrap text', () => {
  const project = structuredClone(fixture.project) as Project;
  const clip = project.clips.find(c => c.content.type === 'text')!;
  if (clip.content.type !== 'text') throw Error('text fixture required');
  clip.content.text = '  説明  A & B\n\n最後  ';
  delete clip.content.layout;
  const original = structuredClone(project);
  const draw = () => render(PreviewVisual, { props: {
    clip, project, time: 0, sessionId: 's', revision: 0, generation: 0,
  } }).body;
  const implicit = draw();
  expect(implicit).toContain('white-space: pre;');
  expect(implicit).toContain('  説明  A &amp; B\n\n最後  ');
  expect(implicit).not.toContain('<tspan');
  expect(project).toEqual(original);
  clip.content.layout = { alignment: 'left', wrap: false };
  expect(draw()).toBe(implicit);
  clip.content.layout = { alignment: 'center', wrap: true };
  expect(draw()).toContain('white-space: pre-wrap;');
  expect(draw()).toContain('text-align: center;');
});

test('free text outline reaches the preview and can be removed without changing content', () => {
  const project = structuredClone(fixture.project) as Project;
  const clip = project.clips.find(c => c.content.type === 'text')!;
  if (clip.content.type !== 'text') throw Error('text fixture required');
  const text = clip.content.text;
  clip.content.outline = { width: 4, color: '#123456' };
  const draw = () => render(PreviewVisual, { props: {
    clip, project, time: 0, sessionId: 's', revision: 0, generation: 0,
  } }).body;
  expect(draw()).toContain('--stroke: 8px #123456');
  delete clip.content.outline;
  expect(draw()).toContain('--stroke: 0px transparent');
  expect(clip.content.text).toBe(text);
});
