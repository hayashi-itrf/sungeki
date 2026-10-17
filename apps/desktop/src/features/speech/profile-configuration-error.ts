/** Only the explicit missing-configuration error carries a setup destination. */
export function missingConfigurationPath(error: unknown): string | null {
  if (!error || typeof error !== 'object' || !('code' in error)
    || error.code !== 'CAPABILITY_UNAVAILABLE' || !('details' in error)) return null;
  const details = error.details;
  if (!details || typeof details !== 'object' || !('configuration_path' in details)) return null;
  const path = details.configuration_path;
  return typeof path === 'string' && path.startsWith('/') && !/[\u0000-\u001f\u007f]/.test(path) ? path : null;
}
