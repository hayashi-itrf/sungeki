import { locale } from './i18n/locale.svelte';

export function editErrorMessage(error: unknown): string {
  if (error && typeof error === 'object' && 'code' in error
    && error.code === 'VALIDATION_FAILED' && 'details' in error) {
    const details = error.details;
    if (details && typeof details === 'object' && 'reason' in details
      && details.reason === 'track_overlap' && 'track_name' in details
      && typeof details.track_name === 'string') {
      return locale.text('editorTrackOverlap').replace('{track}', () => details.track_name as string);
    }
  }
  return error && typeof error === 'object' && 'message' in error ? String(error.message) : String(error);
}
