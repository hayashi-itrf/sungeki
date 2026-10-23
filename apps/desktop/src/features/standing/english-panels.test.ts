import { afterEach, expect, test } from 'vitest';
import { render } from 'svelte/server';
import type { PsdProbe, VrmStanding } from '@sungeki/api-client';
import { locale } from '../../shared/i18n/locale.svelte';
import PsdPoseFields from './PsdPoseFields.svelte';
import PsdSpeechFields from './PsdSpeechFields.svelte';
import VrmFields from './VrmFields.svelte';
import VrmPoseFields from './VrmPoseFields.svelte';
import GainFields from '../speech/GainFields.svelte';
afterEach(() => { locale.current = 'ja'; });

test('English standing and gain panels render translated instructions without modifying settings', () => {
  const probe = { layers: [] } as unknown as PsdProbe;
  const value = { model_asset_id: 'model', motion_asset_id: 'motion', source_start_us: 0, expressions: {}, character_id: null } as VrmStanding;
  const before = JSON.stringify(value);
  locale.current = 'en';
  const output = [
    render(PsdPoseFields, { props: { probe, pose: { source_sha256: "0".repeat(64), selections: [] } } }).body,
    render(PsdSpeechFields, { props: { probe, characters: [], speech: null } }).body,
    render(VrmFields, { props: { value, assets: [], characters: [] } }).body,
    render(VrmPoseFields, { props: { value, assets: [], sessionId: 'test', revision: 0, commit: async () => true } }).body,
    render(GainFields, { props: { enabled: true, gainDb: -3 } }).body,
  ].join('\n');
  expect(output).toContain('Expressions and poses');
  expect(output).toContain('Character lip sync');
  expect(output).toContain('Edit pose visually');
  expect(output).toContain('Adjust speech volume');
  expect(output).not.toMatch(/[ぁ-んァ-ヶ一-龠]/);
  expect(JSON.stringify(value)).toBe(before);
});
