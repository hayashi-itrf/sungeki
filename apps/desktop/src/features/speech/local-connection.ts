import type { LocalSpeechEngine } from '@sungeki/api-client';
export function localEndpoint(engine: LocalSpeechEngine): string {
  try { return localStorage.getItem(`sungeki.${engine}.endpoint`) || defaultEndpoint(engine); }
  catch { return defaultEndpoint(engine); }
}
export function saveLocalEndpoint(engine: LocalSpeechEngine, endpoint: string) {
  try { localStorage.setItem(`sungeki.${engine}.endpoint`, endpoint); } catch { /* In-memory selection still works. */ }
}
function defaultEndpoint(engine: LocalSpeechEngine) { return `http://127.0.0.1:${engine === 'aivis_speech' ? 10101 : 50032}/`; }
