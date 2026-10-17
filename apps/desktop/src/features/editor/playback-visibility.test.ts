import { afterEach, expect, test, vi } from 'vitest';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { playbackVisibility } from './playback-visibility';
vi.mock('@tauri-apps/api/core', () => ({ isTauri: vi.fn() }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: vi.fn() }));
afterEach(() => { vi.unstubAllGlobals(); vi.resetAllMocks(); });
test('browser diagnostics do not call native APIs', async () => {
  vi.stubGlobal('document', { visibilityState: 'visible', hasFocus: () => true });
  vi.mocked(isTauri).mockReturnValue(false);
  expect((await playbackVisibility()).native).toBeNull();
  expect(getCurrentWindow).not.toHaveBeenCalled();
});
test('partial native errors remain unknown and differing web state is preserved', async () => {
  const doc = { visibilityState: 'hidden', hasFocus: () => false };
  vi.stubGlobal('document', doc); vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(getCurrentWindow).mockReturnValue({
    isVisible: async () => { doc.visibilityState = 'visible'; return true; },
    isMinimized: async () => false,
    isFocused: async () => { throw new Error('unavailable'); },
  } as unknown as ReturnType<typeof getCurrentWindow>);
  const report = await playbackVisibility();
  expect(report.before.visibility).toBe('hidden');
  expect(report.after.visibility).toBe('visible');
  expect(report.native).toEqual({ visible: { value: true }, minimized: { value: false },
    focused: { value: null, error: 'Error: unavailable' } });
});
