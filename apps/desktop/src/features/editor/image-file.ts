import { locale } from '../../shared/i18n/locale.svelte';
import { open } from '@tauri-apps/plugin-dialog';
import { hostApi } from '../../shared/host-api';
export async function choosePsd() {
  const path = await open({ title: locale.text('psdAdd'), multiple: false, directory: false,
    filters: [{ name: locale.text('psdFilter'), extensions: ['psd'] }] });
  if (!path) return null;
  const result = await hostApi.session({ type: 'probe_psd', path });
  if (result.type !== 'psd_probed') throw new Error(locale.text('psdProbeFailed'));
  return { path, probe: result.probe, kind: 'psd' as const };
}
export async function chooseImage(title: string) {
  const path = await open({ title, multiple: false, directory: false,
    filters: [{ name: locale.text('imageFilter'), extensions: ['png', 'jpg', 'jpeg', 'webp'] }] });
  if (!path) return null;
  const result = await hostApi.session({ type: 'probe_image', path });
  if (result.type !== 'image_probed') throw new Error(locale.text('imageProbeFailed'));
  return { path, probe: result.probe };
}
export function imageError(error: unknown): string {
  if (error && typeof error === 'object' && 'code' in error && error.code === 'ASSET_MISSING') {
    return locale.text('imageMissing');
  }
  return error && typeof error === 'object' && 'message' in error ? String(error.message) : String(error);
}
