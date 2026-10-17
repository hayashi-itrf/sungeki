import { expect, test } from 'vitest';
import type { ProxyJob, SessionSnapshot } from '@sungeki/api-client';
import { ProxySelection } from './proxy-selection.svelte';
const snapshot = { session_id: 's', revision: 3, project: { assets: [
  { id: 'a', probe: { type: 'video' } }, { id: 'b', probe: { type: 'video' } },
  { id: 'image', probe: { type: 'image' } },
] } } as SessionSnapshot;
const job = { session_id: 's', job_id: 'j', asset_ids: ['a', 'b', 'image'],
  completed: [{ asset_id: 'a' }, { asset_id: 'image' }], source_revision: 2, stale: true } as ProxyJob;
test('only completed video assets from the same session can be selected', () => {
  const selection = new ProxySelection();
  const before = JSON.stringify(snapshot);
  expect(selection.use(snapshot, job, 'b')).toBe(false);
  expect(selection.use(snapshot, job, 'image')).toBe(false);
  expect(selection.use(snapshot, { ...job, session_id: 'other' }, 'a')).toBe(false);
  expect(selection.use(snapshot, job, 'a')).toBe(true); // Host revalidates stale job bytes/probe.
  expect(selection.forSession('s')).toEqual({ a: 'j' });
  expect(JSON.stringify(snapshot)).toBe(before);
});
test('choices do not cross sessions and are discarded on session changes', () => {
  const selection = new ProxySelection(); selection.use(snapshot, job, 'a');
  expect(selection.forSession('other')).toEqual({});
  selection.original('other', 'a'); expect(selection.forSession('s')).toEqual({ a: 'j' });
  selection.use({ ...snapshot, session_id: 'other' }, { ...job, session_id: 'other' }, 'a');
  expect(selection.forSession('s')).toEqual({});
  selection.clear(); expect(selection.forSession('other')).toEqual({});
});
test('restoring one original does not reset another material choice', () => {
  const selection = new ProxySelection(); selection.use(snapshot, job, 'a');
  selection.use(snapshot, { ...job, job_id: 'second', completed: [{ asset_id: 'b' }] } as ProxyJob, 'b');
  selection.original('s', 'a'); expect(selection.forSession('s')).toEqual({ b: 'second' });
});
