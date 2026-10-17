import { expect, test, vi } from 'vitest';
import { missingConfigurationPath } from './profile-configuration-error';
import { Sbv2Profiles } from './profiles.svelte';
import { QwenProfiles } from './qwen-profiles.svelte';
import { IrodoriProfiles } from './irodori-profiles.svelte';
const missing = { code: 'CAPABILITY_UNAVAILABLE', message: 'missing', details: { configuration_path: '/runtime/sbv2.json' } };
test('setup guidance requires structured missing-configuration evidence', () => {
  expect(missingConfigurationPath(missing)).toBe('/runtime/sbv2.json');
  for (const error of [null, 'missing', { ...missing, code: 'IO_ERROR' },
    { ...missing, details: null }, { ...missing, details: { configuration_path: '../config' } },
    { ...missing, details: { configuration_path: '/runtime/\nconfig' } }]) {
    expect(missingConfigurationPath(error)).toBeNull();
  }
});
test('all engine selectors clear stale setup guidance on the next refresh', async () => {
  for (const Profiles of [Sbv2Profiles, QwenProfiles, IrodoriProfiles]) {
    const session = vi.fn().mockRejectedValueOnce(missing)
      .mockRejectedValueOnce({ code: 'IO_ERROR', message: 'permission denied' });
    const profiles = new Profiles({ session });
    await profiles.refresh();
    expect(profiles.configurationPath).toBe('/runtime/sbv2.json');
    expect(profiles.ready).toBe(false);
    await profiles.refresh();
    expect(profiles.configurationPath).toBeNull();
    expect(profiles.error).toBe('permission denied');
    profiles.dispose();
  }
});
