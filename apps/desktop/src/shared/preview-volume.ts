const key = 'sungeki.ui.preview-volume';
export function readPreviewVolume(): number {
  try {
    const raw = localStorage.getItem(key);
    if (raw === null || raw.trim() === '') return 1;
    const value = Number(raw);
    return Number.isFinite(value) && value >= 0 && value <= 1 ? value : 1;
  } catch { return 1; }
}
export function savePreviewVolume(value: number): void {
  if (!Number.isFinite(value) || value < 0 || value > 1) return;
  try { localStorage.setItem(key, String(value)); } catch { /* Keep the current window usable. */ }
}
