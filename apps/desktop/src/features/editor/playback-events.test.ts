import { expect, test } from 'vitest';
import { playbackEvents } from './playback-events';

test('records hidden end/restart state in order and detaches on stop', () => {
  const media = Object.assign(new EventTarget(), { currentTime: 115, paused: false,
    ended: false, seeking: false, readyState: 4, networkState: 1 });
  let editor = 115, now = 0;
  const trace = playbackEvents(media, () => editor, () => 'hidden', () => now++);
  media.ended = true; media.paused = true;
  media.dispatchEvent(new Event('ended'));
  media.currentTime = 0; media.seeking = true;
  media.dispatchEvent(new Event('seeking'));
  editor = 0; media.seeking = false;
  media.dispatchEvent(new Event('seeked'));
  trace.stop(); trace.stop();
  media.dispatchEvent(new Event('play'));
  const report = trace.report();
  expect(report.total).toBe(5);
  expect(report.events[1]).toMatchObject({ event: 'ended', paused: true, ended: true, visibility: 'hidden' });
  expect(report.events[2]).toMatchObject({ event: 'seeking', mediaTime: 0, editorTime: 115 });
  expect(report.events[3]).toMatchObject({ event: 'seeked', mediaTime: 0, editorTime: 0 });
});

test('long playback retains a bounded tail and reports omitted observations', () => {
  const media = Object.assign(new EventTarget(), { currentTime: 0, paused: false,
    ended: false, seeking: false, readyState: 4, networkState: 1 });
  const trace = playbackEvents(media, () => 0, () => 'visible', () => 0);
  for (let i = 0; i < 1000; i++) media.dispatchEvent(new Event('timeupdate'));
  trace.stop();
  expect(trace.report()).toMatchObject({ total: 1002, retained: 256, limit: 256 });
  expect(trace.report().events.at(-1)).toMatchObject({ event: 'diagnostic_stop' });
});
