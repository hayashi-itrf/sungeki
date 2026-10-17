import type { SessionSnapshot } from '@sungeki/api-client';
// EditorController owns deep reactive proxies. Capture plain, detached data
// before cloning a voice or keeping a revision-bound review draft.
export function captureVoiceTuning(snapshot: SessionSnapshot | null): SessionSnapshot | null {
  return $state.snapshot(snapshot);
}
