import { expect, test } from 'vitest';
import type { CollectionJob, SessionSnapshot } from '@sungeki/api-client';
import { canCollect, destination } from './presentation';
const snapshot = { session_id: 's', revision: 5 } as SessionSnapshot;
const job = { session_id: 's', source_revision: 5, phase: 'succeeded', dry_run: true,
  destination: '/tmp/作品', include_checkpoints: false, stale: false, plan: { sources_ready: true, destination_exists: false } } as CollectionJob;
test('only the exact reviewed project revision and destination enable collection', () => {
  expect(canCollect(job, snapshot, '/tmp/作品')).toBe(true);
  for (const override of [{ phase: 'running' }, { include_checkpoints: true }, { dry_run: false }, { stale: true }, { source_revision: 4 },
    { session_id: 'other' }, { plan: null }, { plan: { ...job.plan!, sources_ready: false } },
    { plan: { ...job.plan!, destination_exists: true } }] as Partial<CollectionJob>[]) {
    expect(canCollect({ ...job, ...override }, snapshot, '/tmp/作品')).toBe(false);
  }
  expect(canCollect(job, snapshot, '/tmp/別作品')).toBe(false);
  expect(canCollect(null, snapshot, '/tmp/作品')).toBe(false);
});
test('folder construction rejects escape names while supporting Japanese and root folders', () => {
  expect(destination('/tmp/', ' 作品 ')).toBe('/tmp/作品');
  expect(destination('/', '作品')).toBe('/作品');
  for (const name of ['', '.', '..', '../other', 'a/b', 'a\\b', 'a:b', 'a\0b']) expect(destination('/tmp', name)).toBeNull();
  expect(destination('relative', '作品')).toBeNull();
});

test('history option must match the reviewed job in both directions', () => {
  const withHistory = { ...job, include_checkpoints: true };
  expect(canCollect(withHistory, snapshot, '/tmp/作品', true)).toBe(true);
  expect(canCollect(withHistory, snapshot, '/tmp/作品', false)).toBe(false);
  expect(canCollect(job, snapshot, '/tmp/作品', true)).toBe(false);
  expect(canCollect({ ...withHistory, stale: true }, snapshot, '/tmp/作品', true)).toBe(false);
});
