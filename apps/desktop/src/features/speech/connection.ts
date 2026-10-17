// UI connection preference, separate from the saved Project and its history.
const key = 'sungeki.ui.voicevox-url';
const fallback = 'http://127.0.0.1:50021';
export function voicevoxUrl(): string {
  try { return localStorage.getItem(key) || fallback; }
  catch { return fallback; }
}
// Remember only a connection whose discovery succeeded through the common host.
export function rememberVoicevoxUrl(url: string): void {
  try { localStorage.setItem(key, url); }
  catch { /* Storage restrictions must not prevent character registration. */ }
}
