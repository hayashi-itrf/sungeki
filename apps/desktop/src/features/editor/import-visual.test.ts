import { expect, test } from 'vitest';
import type { Project } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/psd-project.json';
import { importVisual } from './import-visual';

const project = fixture as Project;
const asset = project.assets[0];
if (asset.probe.type !== 'psd') throw new Error('PSD fixture required');
const probe = asset.probe.info;
const ids = { asset: 'new-asset', clip: 'new-clip', track: 'new-track' };

test('ordinary images keep EXIF orientation and use a separate front layer', () => {
  const layered = structuredClone(project);
  layered.tracks.push(
    { id: 'foreground', name: 'foreground', kind: 'visual', visible: true, muted: false },
    { id: 'hidden', name: 'hidden', kind: 'visual', visible: false, muted: false },
    { id: 'audio', name: 'audio', kind: 'audio', visible: true, muted: false },
  );
  const before = structuredClone(layered);
  const commands = importVisual(layered, { kind: 'image', path: '/rotated.jpg', probe: {
    format: 'jpeg', width: 400, height: 200, orientation: 6, has_alpha: false,
    byte_length: 100, sha256: 'a'.repeat(64),
  } }, 0, ids);
  const add = commands.at(-1);
  if (add?.type !== 'add_clip') throw new Error('clip required');
  expect(add.clip.transform.width / add.clip.transform.height).toBeCloseTo(0.5);
  expect(add.clip.track_id).toBe(ids.track);
  expect(commands[1]).toMatchObject({ type: 'add_track', track: { id: ids.track, visible: true } });
  expect(layered).toEqual(before);
});

test('image import creates a visible track when existing visual tracks are hidden', () => {
  const hidden = structuredClone(project);
  hidden.tracks.forEach(t => { t.visible = false; });
  const before = structuredClone(hidden);
  const commands = importVisual(hidden, { kind: 'image', path: '/picture.png', probe: {
    format: 'png', width: 400, height: 200, orientation: 1, has_alpha: true,
    byte_length: 100, sha256: 'b'.repeat(64),
  } }, 0, ids);
  expect(commands.map(c => c.type)).toEqual(['register_asset', 'add_track', 'add_clip']);
  expect(commands[1]).toMatchObject({ track: { id: ids.track, visible: true } });
  expect(commands[2]).toMatchObject({ clip: { track_id: ids.track } });
  expect(hidden).toEqual(before);
});

test('standing import adds a frontmost track and preserves catalog without mutating the project', () => {
  const before = structuredClone(project);
  const commands = importVisual(project, { kind: 'psd', path: '/source/standing.psd', probe }, 1230000, ids);
  expect(commands.map(c => c.type)).toEqual(['register_asset', 'add_track', 'add_clip']);
  const add = commands[2];
  if (add.type !== 'add_clip') throw new Error('clip required');
  expect(add.clip.track_id).toBe(ids.track);
  expect(add.clip.start_us).toBe(1230000);
  expect(add.clip.content).toEqual({ type: 'psd', asset_id: ids.asset,
    pose: { source_sha256: probe.sha256, selections: [] }, speech: null });
  expect(project).toEqual(before);
});

test('portrait standing fits the canvas without cropping or distortion', () => {
  const commands = importVisual(project, { kind: 'psd', path: '/portrait.psd',
    probe: { ...probe, width: 1082, height: 1650 } }, 0, ids);
  const add = commands.at(-1);
  if (add?.type !== 'add_clip') throw new Error('clip required');
  const t = add.clip.transform;
  expect(t.width / t.height).toBeCloseTo(1082 / 1650);
  expect(t.width).toBeLessThanOrEqual(project.canvas.width);
  expect(t.height).toBeLessThanOrEqual(project.canvas.height);
  expect(t.x * 2 + t.width).toBeCloseTo(project.canvas.width);
  expect(t.y * 2 + t.height).toBeCloseTo(project.canvas.height);
});
