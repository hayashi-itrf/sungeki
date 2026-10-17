import { convertFileSrc } from '@tauri-apps/api/core';
export function audioPreviewUrl(jobId: string): string {
  if (!/^[0-9a-f-]{36}$/.test(jobId)) throw new Error('確認音声のIDが不正です');
  return convertFileSrc(jobId, 'audiopreview');
}
