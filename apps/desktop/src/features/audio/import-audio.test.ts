import { expect, test } from 'vitest';
import { importAudio } from './import-audio';
const probe = { sample_rate: 44100, channels: 2, frames: 101, duration_us: 2291, byte_length: 448, sha256: 'a'.repeat(64) };
const ids = { asset: 'asset', track: 'track', clip: 'clip' };
test('places the entire inspected original on a separate audible track without synthesis metadata', () => {
  const before = structuredClone(probe);
  const commands = importAudio('/素材/音楽.wav', probe, 1234567, ids);
  const [asset, track, clip] = commands;
  expect(asset.type).toBe('register_asset');
  expect(track).toEqual({ type: 'add_track', track: { id: 'track', name: '音楽.wav', kind: 'audio', visible: true, muted: false } });
  if (clip.type !== 'add_clip' || clip.clip.content.type !== 'audio') throw new Error('audio required');
  expect(clip.clip.start_us).toBe(1234567);
  expect(clip.clip.duration_us).toBe(2291);
  expect(clip.clip.scene_id).toBeNull();
  expect(clip.clip.content.playback).toEqual({ source_start_us: 0, source_duration_us: 2291, looped: false, gain_db: 0, volume_points: [], fade_in_us: 0, ducking: null, fade_out_us: 0 });
  expect(probe).toEqual(before);
});
test('does not silently shorten imported audio at the timeline limit', () => {
  for (const start of [-1, NaN, 0.5, 604800000000]) expect(() => importAudio('/audio.wav', probe, start, ids)).toThrow();
  expect(() => importAudio('/audio.wav', probe, 604800000000 - probe.duration_us, ids)).not.toThrow();
});
test('retains compressed-source and decoded identities when importing MP3', () => {
  const mp3 = { ...probe, sample_rate: 48000, frames: 144000, duration_us: 3000000,
    encoding: { type: 'mp3' as const, decode_version: 1, decoded_sha256: 'b'.repeat(64) } };
  const commands = importAudio('/music.mp3', mp3, 0, ids);
  const registration = commands[0];
  if (registration.type !== 'register_asset') throw new Error('registration required');
  expect(registration.asset.probe).toEqual({ type: 'audio', info: mp3 });
  expect(registration.asset.content_hash).toBe(mp3.sha256);
});
