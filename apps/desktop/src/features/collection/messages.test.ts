import { afterEach, expect, test } from 'vitest';
import { render } from 'svelte/server';
import type { CollectionJob, CollectionPlanPage } from '@sungeki/api-client';
import { locale } from '../../shared/i18n/locale.svelte';
import { collectionText } from './messages';
import { CollectionJobsController } from './jobs.svelte';
import PlanItems from './PlanItems.svelte';
afterEach(() => { locale.current = 'ja'; });

test('localized placeholders preserve user names without interpreting nested placeholders', () => {
  expect(collectionText('保存時点: {name}', { name: '編集前 {count}' }, 'en')).toBe('Checkpoint: 編集前 {count}');
  expect(collectionText('保存時点を含む（{count}件）', { count: 0 }, 'ja')).toBe('保存時点を含む（0件）');
});

test('English asset results retain missing status, checkpoint identity and escaped user content', () => {
  locale.current = 'en';
  const jobs = new CollectionJobsController({ session: async () => { throw new Error('No request expected'); } }, 'session');
  jobs.selected = { plan: {} } as CollectionJob;
  jobs.page = { total: 1, offset: 0, items: [{ checkpoint_id: 'checkpoint', asset_id: 'asset',
    status: 'missing', byte_length: 74, conflicting_metadata: true, source_path: '/missing.png',
    source_path_truncated: false, detail: null }] } as CollectionPlanPage;
  const html = render(PlanItems, { props: { jobs, checkpointNames: { checkpoint: '<img> {name}' } } }).body;
  expect(html).toContain('Missing');
  expect(html).toContain('Conflicting registered metadata');
  expect(html).toContain('Checkpoint: &lt;img> {name}');
  expect(html).toContain('ID: checkpoint');
  expect(html).toContain('1–1 of 1');
  expect(html).not.toContain('<img>');
  expect(html).not.toContain('見つからない');
  jobs.dispose();
});
