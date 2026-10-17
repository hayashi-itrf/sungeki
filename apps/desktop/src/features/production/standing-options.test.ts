import { expect, test } from 'vitest';
import type { Project } from '@sungeki/api-client';
import { standingOptions } from './standing-options';

test('standing choices filter PSD speakers and collapse equivalent clips without losing variants', () => {
  const content = { type: 'psd', asset_id: 'asset', pose: { selections: [] }, speech: { character_id: 'left' } };
  const base = { id: 'original', content, transform: { flip_x: false }, start_us: 0 };
  const project = { clips: [base,
    { ...base, id: 'later', start_us: 500000 },
    { ...base, id: 'flipped', transform: { flip_x: true } },
    { ...base, id: 'pose', content: { ...content, pose: { selections: [{ group: 'g', option: 'o' }] } } },
    { ...base, id: 'other', content: { ...content, speech: { character_id: 'right' } } },
    { ...base, id: 'unbound', content: { ...content, speech: null } },
    { ...base, id: 'image', content: { type: 'image', asset_id: 'static' } },
  ] } as unknown as Project;
  const before = JSON.stringify(project);
  expect(standingOptions(project, '')).toEqual([]);
  expect(standingOptions(project, 'left').map(c => c.id)).toEqual(['original', 'flipped', 'pose', 'image']);
  expect(standingOptions(project, 'right').map(c => c.id)).toEqual(['other', 'image']);
  expect(JSON.stringify(project)).toBe(before);
});

test('VRM choices require the selected speaker and preserve expression and motion variants', () => {
  const standing = { model_asset_id: 'model', motion_asset_id: 'motion', source_start_us: 0, expressions: { happy: 0.5 }, character_id: 'left' };
  const base = { id: 'vrm', content: { type: 'vrm', standing }, transform: {} };
  const project = { clips: [base, { ...base, id: 'same' },
    { ...base, id: 'other', content: { type: 'vrm', standing: { ...standing, character_id: 'right' } } },
    { ...base, id: 'unbound', content: { type: 'vrm', standing: { ...standing, character_id: null } } },
    { ...base, id: 'offset', content: { type: 'vrm', standing: { ...standing, source_start_us: 100000 } } },
  ] } as unknown as Project;
  expect(standingOptions(project, 'left').map(c => c.id)).toEqual(['vrm', 'offset']);
});
