import { expect, test, vi } from 'vitest';
import { VideoFrameLoop } from './video-frame-loop';
function source() {
  let next = 0;
  const callbacks = new Map<number, VideoFrameRequestCallback>();
  return {
    requestVideoFrameCallback: vi.fn((cb: VideoFrameRequestCallback) => { callbacks.set(++next, cb); return next; }),
    cancelVideoFrameCallback: vi.fn(),
    deliver: (id: number) => callbacks.get(id)!(0, {} as VideoFrameCallbackMetadata),
  };
}
test('does not read pixels before decoded delivery and requests one next frame', () => {
  const video = source(), paint = vi.fn();
  const loop = new VideoFrameLoop(paint, () => true);
  loop.start(video); expect(paint).not.toHaveBeenCalled();
  video.deliver(1); expect(paint).toHaveBeenCalledTimes(1);
  expect(video.requestVideoFrameCallback).toHaveBeenCalledTimes(2);
  loop.stop(); expect(video.cancelVideoFrameCallback).toHaveBeenCalledWith(2);
  video.deliver(2); expect(paint).toHaveBeenCalledTimes(1);
  expect(video.requestVideoFrameCallback).toHaveBeenCalledTimes(2);
});
test('late callbacks from before seek cannot repaint or replace the resumed subscription', () => {
  const video = source(), paint = vi.fn();
  const loop = new VideoFrameLoop(paint, () => true);
  loop.start(video); loop.stop(); loop.start(video);
  video.deliver(1); expect(paint).not.toHaveBeenCalled();
  video.deliver(2); expect(paint).toHaveBeenCalledTimes(1);
  loop.stop(); expect(video.cancelVideoFrameCallback).toHaveBeenLastCalledWith(3);
});
test('pause or a fatal paint failure cannot rearm the loop', () => {
  const video = source(); let eligible = true;
  const paint = vi.fn(() => loop.stop());
  const loop = new VideoFrameLoop(paint, () => eligible);
  loop.start(video); video.deliver(1);
  expect(video.requestVideoFrameCallback).toHaveBeenCalledTimes(1);
  loop.start(video); eligible = false; video.deliver(2);
  expect(paint).toHaveBeenCalledTimes(1);
  expect(video.requestVideoFrameCallback).toHaveBeenCalledTimes(2);
  loop.start(video); expect(video.requestVideoFrameCallback).toHaveBeenCalledTimes(2);
});
