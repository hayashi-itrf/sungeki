import { afterEach, expect, test, vi } from 'vitest';
import { exclusivePlayback } from './exclusive-playback';
class Media extends EventTarget {
  paused = true;
  currentTime = 12;
  constructor(readonly ownerDocument: EventTarget) { super(); }
  pause() { this.paused = true; }
  play() {
    this.paused = false;
    const event = new Event('play');
    Object.defineProperty(event, 'target', { value: this });
    this.ownerDocument.dispatchEvent(event);
  }
}
afterEach(() => vi.unstubAllGlobals());
test('audio and video interrupt each other in both directions without resetting position', () => {
  vi.stubGlobal('HTMLMediaElement', Media);
  const document = new EventTarget();
  const audio = new Media(document), video = new Media(document);
  const a = exclusivePlayback(audio as unknown as HTMLMediaElement);
  const v = exclusivePlayback(video as unknown as HTMLMediaElement);
  audio.play(); video.play();
  expect(audio.paused).toBe(true); expect(video.paused).toBe(false);
  audio.play();
  expect(video.paused).toBe(true); expect(audio.paused).toBe(false);
  expect(video.currentTime).toBe(12); expect(audio.currentTime).toBe(12);
  a.destroy(); v.destroy();
});
test('removal stops playback and unregisters the listener; unrelated events do not interrupt', () => {
  vi.stubGlobal('HTMLMediaElement', Media);
  const document = new EventTarget(), audio = new Media(document);
  const action = exclusivePlayback(audio as unknown as HTMLMediaElement);
  audio.play(); document.dispatchEvent(new Event('play'));
  expect(audio.paused).toBe(false);
  action.destroy(); expect(audio.paused).toBe(true);
  const pause = vi.spyOn(audio, 'pause');
  new Media(document).play(); expect(pause).not.toHaveBeenCalled();
});
