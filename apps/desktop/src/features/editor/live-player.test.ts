import { afterEach, expect, test, vi } from 'vitest';
import type { SessionSnapshot } from '@sungeki/api-client';
import fixture from '../../../../../tests/fixtures/scene-evaluation.json';
import { LivePlayer } from './live-player';
const snapshot = () => ({ session_id: 's', revision: 1, event_cursor: { host_instance_id: 'host', event_seq: 0 }, document_path: null, dirty: false, undo_operation_id: null, redo_operation_id: null, project: structuredClone(fixture.project) }) as SessionSnapshot;
class Context {
  static latest: Context;
  currentTime = 0;
  sources: { start: ReturnType<typeof vi.fn>; stop: ReturnType<typeof vi.fn>; connect: ReturnType<typeof vi.fn>; disconnect: ReturnType<typeof vi.fn> }[] = [];
  createBufferSource = () => {
    const source = { start: vi.fn(), stop: vi.fn(), connect: vi.fn(), disconnect: vi.fn() };
    this.sources.push(source); return source;
  };
  decodeAudioData = async () => ({ duration: 4, length: 192000, numberOfChannels: 1 });
  destination = {};
  constructor() { Context.latest = this; }
  resume = vi.fn(async () => {});
  close = vi.fn(async () => {});
  createGain = vi.fn(() => ({ gain: { value: 1, setTargetAtTime: vi.fn() }, connect: vi.fn(), disconnect: vi.fn() }));
}
function setup(audio = false) {
  let frame: FrameRequestCallback | undefined;
  vi.stubGlobal('AudioContext', Context);
  vi.stubGlobal('document', { dispatchEvent: vi.fn() });
  vi.stubGlobal('requestAnimationFrame', (f: FrameRequestCallback) => { frame = f; return 1; });
  vi.stubGlobal('cancelAnimationFrame', () => { frame = undefined; });
  const update = vi.fn(), fail = vi.fn(), api = { session: vi.fn() };
  const s = snapshot(); s.project.clips = s.project.clips.filter(c => c.content.type === 'shape');
  s.project.clips[0].start_us = 0; s.project.clips[0].duration_us = 10e6;
  if (audio) {
    s.project.tracks.forEach(t => t.muted = false);
    s.project.clips[0].content = { type: 'audio', asset_id: 'a', playback: {
      source_start_us: 0, source_duration_us: 4e6, looped: true, gain_db: -6,
      fade_in_us: 0, fade_out_us: 0, volume_points: [], ducking: null,
    } };
    api.session.mockResolvedValue({ type: 'editing_audio_bytes', offset: 0, total: 1, base64: 'YQ==' });
  }
  const player = new LivePlayer(api, s, update, fail);
  return { player, update, fail, api, advance: (time: number) => { Context.latest.currentTime = time; const f = frame; frame = undefined; f?.(time * 1000); } };
}
afterEach(() => vi.unstubAllGlobals());
test('plays immediately without any host rendering, pauses in place and resumes', async () => {
  const { player, update, api, advance } = setup();
  await player.play(2); advance(1);
  expect(update).toHaveBeenLastCalledWith(3, true, false);
  player.pause(); advance(8);
  expect(update).toHaveBeenLastCalledWith(3, false, false);
  await player.play(3); advance(9);
  expect(update).toHaveBeenLastCalledWith(4, true, false);
  expect(api.session).not.toHaveBeenCalled(); player.dispose();
});
test('seek while playing resumes from target; end pauses without resetting to zero', async () => {
  const { player, update, advance } = setup();
  await player.play(0); advance(1); player.seek(7); await Promise.resolve();
  advance(2); expect(update).toHaveBeenLastCalledWith(8, true, false);
  advance(5); expect(update).toHaveBeenLastCalledWith(10, false, false);
  player.dispose();
});
test('dispose cancels a start that has not resumed its audio context yet', async () => {
  const { player, update } = setup();
  const start = player.play(2); update.mockClear(); player.dispose(); await start;
  expect(update).not.toHaveBeenCalled();
  expect(Context.latest.close).toHaveBeenCalledOnce();
});

test('decoded audio pauses with the clock and resumes at the correct source offset', async () => {
  const { player, update, advance } = setup(true);
  await player.play(2);
  expect(Context.latest.sources[0].start).toHaveBeenCalledWith(0, 2, 8);
  advance(1); player.pause();
  expect(Context.latest.sources[0].stop).toHaveBeenCalled();
  expect(update).toHaveBeenLastCalledWith(3, false, false);
  await player.play(3);
  expect(Context.latest.sources[1].start).toHaveBeenCalledWith(1, 3, 7);
  player.dispose();
});
test('pause during loading prevents a late audio response from restarting playback', async () => {
  const { player, update, api } = setup(true);
  let finish!: (value: unknown) => void;
  api.session.mockImplementation(() => new Promise(resolve => finish = resolve));
  const playing = player.play(2);
  for (let i=0; i<10 && !finish; i++) await Promise.resolve();
  expect(update).toHaveBeenLastCalledWith(2, true, true);
  player.pause(); finish({ type: 'editing_audio_bytes', offset: 0, total: 1, base64: 'YQ==' }); await playing;
  expect(Context.latest.sources).toHaveLength(0);
  expect(update).toHaveBeenLastCalledWith(2, false, false); player.dispose();
});


test.each([false, true])('document switch teardown cannot restore the old position (playing=%s)', async playing => {
  const { player, update, advance } = setup(true);
  if (playing) { await player.play(2); advance(1); }
  else player.seek(6);
  // The editor has reset to zero for the next document before old components unmount.
  let nextDocumentTime = 0;
  update.mockImplementation(time => { nextDocumentTime = time; });
  update.mockClear();
  player.dispose(); player.pause(); player.seek(8); await player.play(8);
  if (playing) advance(9);
  expect(nextDocumentTime).toBe(0);
  expect(update).not.toHaveBeenCalled();
  if (playing) {
    expect(Context.latest.sources[0].stop).toHaveBeenCalled();
    expect(Context.latest.close).toHaveBeenCalledOnce();
  }
});


test('a rebuilt player uses the retained audition volume on its first playback', async () => {
  const { player } = setup(true);
  player.setVolume(0.2);
  await player.play(0);
  const master = Context.latest.createGain.mock.results[0].value;
  expect(master.gain.value).toBe(0.2);
  player.pause(); await player.play(1);
  expect(master.gain.value).toBe(0.2);
  player.dispose();
});
