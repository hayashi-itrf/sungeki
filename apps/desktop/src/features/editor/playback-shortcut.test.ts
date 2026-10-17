import { afterEach, expect, test, vi } from 'vitest';
import { playbackShortcut } from './playback-shortcut';
class Target { constructor(private editable: boolean) {} closest() { return this.editable ? this : null; } }
function event(patch = {}) { return { code: 'Space', preventDefault: vi.fn(), target: new Target(false), ...patch } as unknown as KeyboardEvent; }
afterEach(() => vi.unstubAllGlobals());
test('Space toggles exactly once; typing, held keys, IME and modifiers keep their normal behavior', () => {
  vi.stubGlobal('Element', Target); vi.stubGlobal('document', { querySelector: () => null });
  const toggle = vi.fn(), space = event(); playbackShortcut(space, toggle);
  expect(toggle).toHaveBeenCalledOnce(); expect(space.preventDefault).toHaveBeenCalledOnce();
  for (const patch of [{ repeat: true }, { isComposing: true }, { ctrlKey: true }, { altKey: true },
    { metaKey: true }, { shiftKey: true }, { defaultPrevented: true }, { code: 'Enter' }, { target: new Target(true) }]) {
    const key = event(patch); playbackShortcut(key, toggle); expect(key.preventDefault).not.toHaveBeenCalled();
  }
  expect(toggle).toHaveBeenCalledOnce();
});
test('an open modal never starts playback behind the dialog', () => {
  vi.stubGlobal('Element', Target); vi.stubGlobal('document', { querySelector: () => ({}) });
  const toggle = vi.fn(); playbackShortcut(event(), toggle); expect(toggle).not.toHaveBeenCalled();
});
