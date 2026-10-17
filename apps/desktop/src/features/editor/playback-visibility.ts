import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

// Read-only diagnostics. Native visibility is not equivalent to foreground eligibility.
export async function playbackVisibility() {
  const webState = () => ({ visibility: document.visibilityState, focused: document.hasFocus() });
  const before = webState();
  if (!isTauri()) return { before, after: webState(), native: null, note: 'Browser only' };
  const window = getCurrentWindow();
  const keys = ['visible', 'minimized', 'focused'] as const;
  const results = await Promise.allSettled([
    window.isVisible(), window.isMinimized(), window.isFocused(),
  ]);
  const native = Object.fromEntries(results.map((result, index) => [keys[index],
    result.status === 'fulfilled' ? { value: result.value } : { value: null, error: String(result.reason) }]));
  return { before, after: webState(), native,
    note: 'Sequential system observations, not an atomic snapshot; native visibility does not prove unobscured foreground rendering' };
}
